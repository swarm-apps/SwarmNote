//! Document state coordination: hydration (ensure every DB row has
//! `yjs_state`) and closed-doc external-change merge.
//!
//! Runs against raw `Arc<DatabaseConnection>` + `Arc<dyn FileSystem>` —
//! sync-broadcast side-effects are the caller's responsibility (PR #3 will
//! wire `WorkspaceSync` in; PR #2 hosts can invoke these helpers directly).

use std::sync::Arc;

use entity::workspace::documents;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, Transact};

use crate::error::{AppError, AppResult};
use crate::fs::FileSystem;
use crate::yjs::{apply_update_to_doc, content_hash, create_doc, fill_doc_with_markdown};

// ── Public types ────────────────────────────────────────────

/// Progress callback signature (Tauri hosts typically wrap
/// `tauri::ipc::Channel::send`).
pub type HydrateProgressFn = Arc<dyn Fn(HydrateProgress) + Send + Sync + 'static>;

/// Progress tick emitted during hydration.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HydrateProgress {
    pub current: usize,
    pub total: usize,
}

/// Summary returned when hydration completes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HydrateResult {
    pub generated: usize,
    pub merged: usize,
    pub skipped: usize,
    pub failed: usize,
}

// ── Hydration ───────────────────────────────────────────────

/// Ensure every document in `workspace_uuid` has a valid `yjs_state`.
///
/// Must run after file-tree reconcile and before GossipSub subscription.
/// `progress` is invoked once per document before processing; hosts can pass
/// a no-op closure if they don't need progress UI.
pub async fn hydrate_workspace(
    db: &DatabaseConnection,
    fs: &dyn FileSystem,
    workspace_uuid: Uuid,
    progress: &HydrateProgressFn,
) -> AppResult<HydrateResult> {
    let all_docs = documents::Entity::find()
        .filter(documents::Column::WorkspaceId.eq(workspace_uuid))
        .all(db)
        .await?;

    let total = all_docs.len();
    let mut result = HydrateResult {
        generated: 0,
        merged: 0,
        skipped: 0,
        failed: 0,
    };

    for (i, doc_model) in all_docs.into_iter().enumerate() {
        progress(HydrateProgress {
            current: i + 1,
            total,
        });

        match hydrate_single_doc(db, fs, doc_model).await {
            Ok(HydrateAction::Generated) => result.generated += 1,
            Ok(HydrateAction::Merged) => result.merged += 1,
            Ok(HydrateAction::Skipped) => result.skipped += 1,
            Err(e) => {
                tracing::warn!("hydrate failed for doc: {e}");
                result.failed += 1;
            }
        }
    }

    tracing::info!(
        "Hydrate workspace {workspace_uuid}: generated={}, merged={}, skipped={}, failed={}",
        result.generated,
        result.merged,
        result.skipped,
        result.failed
    );

    Ok(result)
}

enum HydrateAction {
    Generated,
    Merged,
    Skipped,
}

async fn hydrate_single_doc(
    db: &DatabaseConnection,
    fs: &dyn FileSystem,
    doc_model: documents::Model,
) -> AppResult<HydrateAction> {
    let md_content = match fs.read_text(&doc_model.rel_path).await {
        Ok(c) => c,
        Err(AppError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HydrateAction::Skipped);
        }
        Err(e) => return Err(e),
    };

    let file_hash = content_hash(&md_content);

    match &doc_model.yjs_state {
        None => {
            let m = generate_doc_state(&md_content, file_hash);
            persist_doc_state(db, doc_model, &m).await?;
            Ok(HydrateAction::Generated)
        }
        Some(_) => {
            if doc_model
                .file_hash
                .as_ref()
                .is_some_and(|h| *h == file_hash)
            {
                return Ok(HydrateAction::Skipped);
            }

            let m = merge_external_change(&doc_model, &md_content, file_hash)?;
            persist_doc_state(db, doc_model, &m).await?;
            Ok(HydrateAction::Merged)
        }
    }
}

// ── Closed-doc external change merge ────────────────────────

/// Outcome of [`merge_closed_doc_change`] — carries the incremental diff so
/// callers can broadcast it via sync layer.
#[derive(Debug)]
pub struct ClosedDocMergeResult {
    pub doc_id: Uuid,
    pub diff: Vec<u8>,
}

/// Merge an external `.md` change into a document that is NOT currently
/// open. Returns `None` if the file is self-written (hash matches), the
/// document has no DB record, or the file was deleted.
///
/// Callers responsible for broadcasting the resulting `diff` via sync layer.
pub async fn merge_closed_doc_change(
    db: &DatabaseConnection,
    fs: &dyn FileSystem,
    workspace_uuid: Uuid,
    rel_path: &str,
) -> AppResult<Option<ClosedDocMergeResult>> {
    let new_content = match fs.read_text(rel_path).await {
        Ok(c) => c,
        Err(AppError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    let doc_model = documents::Entity::find()
        .filter(documents::Column::RelPath.eq(rel_path))
        .filter(documents::Column::WorkspaceId.eq(workspace_uuid))
        .one(db)
        .await?;

    let Some(doc_model) = doc_model else {
        return Ok(None);
    };

    let file_hash = content_hash(&new_content);

    // Self-write detection.
    if doc_model
        .file_hash
        .as_ref()
        .is_some_and(|h| *h == file_hash)
    {
        return Ok(None);
    }

    let doc_id = doc_model.id;
    let m = match &doc_model.yjs_state {
        Some(_) => merge_external_change(&doc_model, &new_content, file_hash)?,
        None => generate_doc_state(&new_content, file_hash),
    };

    let diff = m.diff.clone();
    persist_doc_state(db, doc_model, &m).await?;

    tracing::info!("Closed-doc external change processed: {rel_path} (doc {doc_id})");
    Ok(Some(ClosedDocMergeResult { doc_id, diff }))
}

// ── Helpers ─────────────────────────────────────────────────

/// Computed document state ready for DB persistence and/or broadcast.
struct DocState {
    yjs_state: Vec<u8>,
    state_vector: Vec<u8>,
    file_hash: Vec<u8>,
    /// Incremental diff (equals `yjs_state` for freshly generated docs).
    diff: Vec<u8>,
}

fn generate_doc_state(md_content: &str, file_hash: Vec<u8>) -> DocState {
    let doc = create_doc();
    fill_doc_with_markdown(&doc, md_content);
    let txn = doc.transact();
    let yjs_state = txn.encode_state_as_update_v1(&StateVector::default());
    let state_vector = txn.state_vector().encode_v1();
    let diff = yjs_state.clone();
    DocState {
        yjs_state,
        state_vector,
        file_hash,
        diff,
    }
}

fn merge_external_change(
    doc_model: &documents::Model,
    new_md: &str,
    file_hash: Vec<u8>,
) -> AppResult<DocState> {
    let doc = create_doc();

    if let Some(ref existing) = doc_model.yjs_state {
        // Tolerate legacy BlockNote states — they decode cleanly but leave
        // Y.Text empty. `replace_doc_content` then diffs against an empty
        // string, effectively rebuilding the doc from `new_md`.
        if let Err(e) = apply_update_to_doc(&doc, existing, "existing yjs_state") {
            tracing::warn!("legacy yjs_state decode failed, rebuilding from .md: {e}");
        }
    }

    let sv_before = doc.transact().state_vector();
    super::replace_doc_content(&doc, new_md);

    let txn = doc.transact();
    let diff = txn.encode_state_as_update_v1(&sv_before);
    let yjs_state = txn.encode_state_as_update_v1(&StateVector::default());
    let state_vector = txn.state_vector().encode_v1();

    Ok(DocState {
        yjs_state,
        state_vector,
        file_hash,
        diff,
    })
}

async fn persist_doc_state(
    db: &DatabaseConnection,
    doc_model: documents::Model,
    state: &DocState,
) -> AppResult<()> {
    let mut model: documents::ActiveModel = doc_model.into();
    model.yjs_state = Set(Some(state.yjs_state.clone()));
    model.state_vector = Set(Some(state.state_vector.clone()));
    model.file_hash = Set(Some(state.file_hash.clone()));
    model.update(db).await?;
    Ok(())
}
