//! Per-document sync: StateVector exchange, FullSync pull, closed-doc handling.

use std::sync::Arc;
use std::time::Duration;

use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use swarm_p2p_core::libp2p::PeerId;
use tracing::{info, warn};
use uuid::Uuid;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, Transact};

use crate::app::AppCore;
use crate::error::{AppError, AppResult};
use crate::network::AppNetClient;
use crate::protocol::{AppRequest, AppResponse, SyncRequest, SyncResponse};
use crate::yjs::{apply_update_to_doc, content_hash, create_doc, doc_to_markdown};

/// Apply a remote update to a document, routing through YDocManager if open
/// or falling back to the temporary-Doc DB path.
pub async fn apply_remote_update(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_uuid: Uuid,
    update: &[u8],
) -> AppResult<()> {
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;

    // Try the open-doc path first
    if let Some(result) = ws.ydoc().apply_sync_update(&doc_uuid, update).await {
        return result;
    }

    // Document not open → temporary Doc path
    sync_closed_doc(core, workspace_uuid, doc_uuid, update).await
}

/// Sync a closed document: load from DB, apply remote update, persist back.
async fn sync_closed_doc(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_uuid: Uuid,
    remote_update: &[u8],
) -> AppResult<()> {
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;
    let db: &sea_orm::DatabaseConnection = ws.db();

    use entity::workspace::documents;

    let doc_model = documents::Entity::find_by_id(doc_uuid)
        .one(db)
        .await?
        .ok_or(AppError::DocRowMissing(doc_uuid))?;

    let doc = create_doc();

    if let Some(ref yjs_state) = doc_model.yjs_state {
        // Legacy BlockNote states decode cleanly but leave our Y.Text empty.
        // That's OK — the subsequent remote update will populate Y.Text directly.
        if let Err(e) = apply_update_to_doc(&doc, yjs_state, "existing state") {
            tracing::warn!("legacy yjs_state decode failed, using remote update only: {e}");
        }
    }
    apply_update_to_doc(&doc, remote_update, "remote update")?;

    // Encode new state
    let txn = doc.transact();
    let new_state = txn.encode_state_as_update_v1(&StateVector::default());
    let new_sv = txn.state_vector().encode_v1();
    drop(txn);

    // Convert to markdown and write file
    let markdown = doc_to_markdown(&doc);
    let workspace_path = get_workspace_path(core, workspace_uuid).await?;
    let file_path = workspace_path.join(&doc_model.rel_path);
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    tokio::fs::write(&file_path, &markdown).await?;

    // Persist back to DB
    let file_hash = content_hash(&markdown);
    let mut model: documents::ActiveModel = doc_model.into();
    model.yjs_state = Set(Some(new_state));
    model.state_vector = Set(Some(new_sv));
    model.file_hash = Set(Some(file_hash));
    model.update(db).await?;

    // doc is dropped here — memory reclaimed immediately
    Ok(())
}

/// Exchange StateVector with a peer for an existing document.
pub async fn sync_via_state_vector(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
    doc_id: Uuid,
) -> AppResult<()> {
    // Load local state vector
    let sv = load_local_state_vector(core, workspace_uuid, doc_id).await?;

    // Send SV to peer, receive missing updates
    let request = AppRequest::Sync(SyncRequest::StateVector { doc_id, sv });
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        client.send_request(peer_id, request),
    )
    .await
    .map_err(|_| AppError::SwarmIo {
        context: "send_request StateVector",
        reason: format!("timed out for {doc_id}"),
    })?
    .map_err(|e| AppError::SwarmIo {
        context: "send_request StateVector",
        reason: e.to_string(),
    })?;

    match response {
        AppResponse::Sync(SyncResponse::Updates { doc_id: _, updates }) => {
            if !updates.is_empty() {
                apply_remote_update(core, workspace_uuid, doc_id, &updates).await?;
                info!("Applied SV diff for doc {doc_id} ({} bytes)", updates.len());
            }
        }
        other => {
            warn!("Unexpected SV response for {doc_id}: {other:?}");
        }
    }

    Ok(())
}

/// Pull a complete document from a peer (new document that doesn't exist locally).
pub async fn sync_via_full_pull(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
    doc_id: Uuid,
    rel_path: &str,
) -> AppResult<()> {
    let request = AppRequest::Sync(SyncRequest::FullSync { doc_id });
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        client.send_request(peer_id, request),
    )
    .await
    .map_err(|_| AppError::SwarmIo {
        context: "send_request FullSync",
        reason: format!("timed out for {doc_id}"),
    })?
    .map_err(|e| AppError::SwarmIo {
        context: "send_request FullSync",
        reason: e.to_string(),
    })?;

    let updates = match response {
        AppResponse::Sync(SyncResponse::Updates { doc_id: _, updates }) => updates,
        other => {
            return Err(AppError::SwarmIo {
                context: "FullSync response",
                reason: format!("unexpected response: {other:?}"),
            });
        }
    };

    let doc = create_doc();
    apply_update_to_doc(&doc, &updates, "full pull")?;

    let txn = doc.transact();
    let yjs_state = txn.encode_state_as_update_v1(&StateVector::default());
    let state_vector = txn.state_vector().encode_v1();
    drop(txn);

    let markdown = doc_to_markdown(&doc);

    // Write file
    let workspace_path = get_workspace_path(core, workspace_uuid).await?;
    let file_path = workspace_path.join(rel_path);
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    tokio::fs::write(&file_path, &markdown).await?;

    let file_hash = content_hash(&markdown);

    // Create DB record
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;
    let db = ws.db();

    use entity::workspace::documents;
    let new_doc = documents::ActiveModel {
        id: Set(doc_id),
        workspace_id: Set(workspace_uuid),
        folder_id: Set(None),
        title: Set(title_from_rel_path(rel_path)),
        rel_path: Set(rel_path.to_string()),
        file_hash: Set(Some(file_hash)),
        yjs_state: Set(Some(yjs_state)),
        state_vector: Set(Some(state_vector)),
        lamport_clock: Set(0),
        created_by: Set(String::new()),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    new_doc.insert(db).await?;

    info!("Pulled new doc {doc_id} at {rel_path}");
    Ok(())
}

/// Handle inbound StateVector request: compute diff and respond.
pub async fn handle_state_vector_request(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    pending_id: u64,
    doc_id: Uuid,
    remote_sv_bytes: &[u8],
    workspace_uuid: Uuid,
) -> AppResult<()> {
    let diff = compute_update_for_peer(core, workspace_uuid, doc_id, remote_sv_bytes).await?;

    let resp = AppResponse::Sync(SyncResponse::Updates {
        doc_id,
        updates: diff,
    });
    client
        .send_response(pending_id, resp)
        .await
        .map_err(|e| AppError::SwarmIo {
            context: "send_response StateVector",
            reason: e.to_string(),
        })?;

    Ok(())
}

/// Handle inbound FullSync request: load full state and respond.
pub async fn handle_full_sync_request(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    pending_id: u64,
    doc_id: Uuid,
    workspace_uuid: Uuid,
) -> AppResult<()> {
    let full_state = load_full_doc_state(core, workspace_uuid, doc_id).await?;

    let resp = AppResponse::Sync(SyncResponse::Updates {
        doc_id,
        updates: full_state,
    });
    client
        .send_response(pending_id, resp)
        .await
        .map_err(|e| AppError::SwarmIo {
            context: "send_response FullSync",
            reason: e.to_string(),
        })?;

    Ok(())
}

/// Apply a remote deletion: remove DB record + files.
pub async fn apply_deletion(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_id: Uuid,
    rel_path: &str,
    remote_clock: i64,
) -> AppResult<()> {
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;
    let db = ws.db();

    use entity::workspace::{deletion_log, documents};

    // Write tombstone
    let peer_id_str = core.identity.peer_id().unwrap_or_default();

    let tombstone = deletion_log::ActiveModel {
        doc_id: Set(doc_id),
        rel_path: Set(rel_path.to_string()),
        deleted_at: Set(chrono::Utc::now()),
        deleted_by: Set(peer_id_str),
        lamport_clock: Set(remote_clock),
    };
    // ON CONFLICT update
    use sea_orm::sea_query::OnConflict;
    documents::Entity::delete_by_id(doc_id).exec(db).await.ok();
    deletion_log::Entity::insert(tombstone)
        .on_conflict(
            OnConflict::column(deletion_log::Column::DocId)
                .update_columns([
                    deletion_log::Column::DeletedAt,
                    deletion_log::Column::DeletedBy,
                    deletion_log::Column::LamportClock,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;

    // Delete files
    let workspace_path = get_workspace_path(core, workspace_uuid).await?;
    let md_path = workspace_path.join(rel_path);
    tokio::fs::remove_file(&md_path).await.ok();

    // Delete .assets directory
    let asset_dir = asset_dir_from_rel_path(rel_path);
    let asset_path = workspace_path.join(&asset_dir);
    if asset_path.is_dir() {
        tokio::fs::remove_dir_all(&asset_path).await.ok();
    }

    info!("Applied remote deletion for doc {doc_id} at {rel_path}");
    Ok(())
}

/// Resolve a same-path different-UUID conflict using Lamport clock arbitration.
/// The loser document gets renamed with a numeric suffix.
#[allow(clippy::too_many_arguments)]
pub async fn resolve_path_conflict(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    peer_id: PeerId,
    workspace_uuid: Uuid,
    local_doc_id: Uuid,
    remote_doc_id: Uuid,
    rel_path: &str,
    local_clock: i64,
    remote_clock: i64,
) -> AppResult<()> {
    // Determine winner: higher clock wins; on tie, smaller UUID wins
    let local_is_winner = if local_clock != remote_clock {
        local_clock > remote_clock
    } else {
        local_doc_id < remote_doc_id
    };

    let workspace_path = get_workspace_path(core, workspace_uuid).await?;

    if local_is_winner {
        // Remote is loser — pull it with a renamed path
        let new_path = find_available_name(rel_path, &workspace_path);
        info!(
            "Path conflict: local {local_doc_id} wins, pulling remote {remote_doc_id} as {new_path}"
        );
        sync_via_full_pull(
            core,
            client,
            peer_id,
            workspace_uuid,
            remote_doc_id,
            &new_path,
        )
        .await?;
    } else {
        // Local is loser — rename local, then pull remote with original name
        let new_path = find_available_name(rel_path, &workspace_path);
        info!(
            "Path conflict: remote {remote_doc_id} wins, renaming local {local_doc_id} to {new_path}"
        );
        rename_local_doc(core, workspace_uuid, local_doc_id, rel_path, &new_path).await?;
        sync_via_full_pull(
            core,
            client,
            peer_id,
            workspace_uuid,
            remote_doc_id,
            rel_path,
        )
        .await?;
    }

    Ok(())
}

/// Rename a local document's file, assets directory, and DB record.
async fn rename_local_doc(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_id: Uuid,
    old_rel_path: &str,
    new_rel_path: &str,
) -> AppResult<()> {
    let workspace_path = get_workspace_path(core, workspace_uuid).await?;

    // Rename .md file
    let old_file = workspace_path.join(old_rel_path);
    let new_file = workspace_path.join(new_rel_path);
    if old_file.exists() {
        if let Some(parent) = new_file.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::rename(&old_file, &new_file).await?;
    }

    // Rename .assets directory
    let old_assets = workspace_path.join(asset_dir_from_rel_path(old_rel_path));
    let new_assets = workspace_path.join(asset_dir_from_rel_path(new_rel_path));
    if old_assets.is_dir() {
        tokio::fs::rename(&old_assets, &new_assets).await.ok();
    }

    // Update DB record
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;
    let db = ws.db();

    use entity::workspace::documents;
    if let Some(model) = documents::Entity::find_by_id(doc_id).one(db).await? {
        let mut active: documents::ActiveModel = model.into();
        active.rel_path = Set(new_rel_path.to_string());
        active.title = Set(title_from_rel_path(new_rel_path));
        active.update(db).await?;
    }

    // Update YDocManager if doc is open
    ws.ydoc().rename_doc(doc_id, new_rel_path);

    Ok(())
}

/// Generate a conflict-renamed path: `notes/todo.md` → `notes/todo (1).md`.
/// Checks the filesystem to avoid collisions with existing files.
fn find_available_name(rel_path: &str, workspace_path: &std::path::Path) -> String {
    let path = std::path::Path::new(rel_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("md");
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

    for i in 1..100 {
        let new_name = format!("{stem} ({i}).{ext}");
        let candidate = if parent.is_empty() {
            new_name.clone()
        } else {
            format!("{parent}/{new_name}")
        };
        if !workspace_path.join(&candidate).exists() {
            return candidate;
        }
    }
    // Fallback: should never reach here in practice
    let new_name = format!("{stem} (1).{ext}");
    if parent.is_empty() {
        new_name
    } else {
        format!("{parent}/{new_name}")
    }
}

// ── Helpers ──

/// Derive asset directory from document rel_path.
/// `notes/my-note.md` → `notes/my-note.assets`
pub(crate) fn asset_dir_from_rel_path(rel_path: &str) -> String {
    let base = rel_path.strip_suffix(".md").unwrap_or(rel_path);
    format!("{base}.assets")
}

/// Load the local state vector for a document.
async fn load_local_state_vector(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_id: Uuid,
) -> AppResult<Vec<u8>> {
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;

    // Try YDocManager first (open doc)
    if let Some(sv) = ws.ydoc().get_state_vector(&doc_id).await {
        return Ok(sv);
    }

    // Fall back to DB
    let db: &sea_orm::DatabaseConnection = ws.db();

    use entity::workspace::documents;
    let doc = documents::Entity::find_by_id(doc_id)
        .one(db)
        .await?
        .ok_or(AppError::DocRowMissing(doc_id))?;

    if let Some(ref sv) = doc.state_vector {
        return Ok(sv.clone());
    }

    // Compute from yjs_state if state_vector not cached
    if let Some(ref yjs_state) = doc.yjs_state {
        let tmp_doc = create_doc();
        apply_update_to_doc(&tmp_doc, yjs_state, "load SV")?;
        let txn = tmp_doc.transact();
        return Ok(txn.state_vector().encode_v1());
    }

    // Empty document
    Ok(StateVector::default().encode_v1())
}

/// Compute the update a peer needs given their state vector.
async fn compute_update_for_peer(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_id: Uuid,
    remote_sv_bytes: &[u8],
) -> AppResult<Vec<u8>> {
    let remote_sv = StateVector::decode_v1(remote_sv_bytes).map_err(|e| AppError::YjsDecode {
        context: "state vector",
        reason: e.to_string(),
    })?;

    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;

    // Try YDocManager first
    if let Some(update) = ws.ydoc().encode_diff_for_sv(&doc_id, &remote_sv).await {
        return Ok(update);
    }

    // Fall back to DB
    let db: &sea_orm::DatabaseConnection = ws.db();

    use entity::workspace::documents;
    let doc_model = documents::Entity::find_by_id(doc_id)
        .one(db)
        .await?
        .ok_or(AppError::DocRowMissing(doc_id))?;

    if let Some(ref yjs_state) = doc_model.yjs_state {
        let tmp_doc = create_doc();
        apply_update_to_doc(&tmp_doc, yjs_state, "compute diff")?;
        let txn = tmp_doc.transact();
        return Ok(txn.encode_state_as_update_v1(&remote_sv));
    }

    // Empty document
    Ok(vec![])
}

/// Load the full encoded state of a document.
async fn load_full_doc_state(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
    doc_id: Uuid,
) -> AppResult<Vec<u8>> {
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;

    // Try YDocManager first
    if let Some(state) = ws.ydoc().encode_full_state(&doc_id).await {
        return Ok(state);
    }

    // Fall back to DB
    let db: &sea_orm::DatabaseConnection = ws.db();

    use entity::workspace::documents;
    let doc_model = documents::Entity::find_by_id(doc_id)
        .one(db)
        .await?
        .ok_or(AppError::DocRowMissing(doc_id))?;

    Ok(doc_model.yjs_state.unwrap_or_default())
}

/// Get the filesystem path for a workspace.
pub(crate) async fn get_workspace_path(
    core: &Arc<AppCore>,
    workspace_uuid: Uuid,
) -> AppResult<std::path::PathBuf> {
    let info = core
        .workspace_info(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceDb)?;
    Ok(std::path::PathBuf::from(&info.path))
}

/// Extract a title from a relative path (filename without extension).
fn title_from_rel_path(rel_path: &str) -> String {
    std::path::Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string()
}
