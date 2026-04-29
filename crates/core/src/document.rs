//! Document + folder CRUD against the per-workspace SeaORM DB.
//!
//! [`DocumentCrud`] owns an `Arc<DatabaseConnection>` injected at construction
//! (usually by `WorkspaceCore`), so every method is a plain async call — no
//! `DatabaseConnection` parameter plumbed through each call site.

use std::sync::Arc;

use chrono::Utc;
use entity::workspace::{deletion_log, documents, folders};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// Extract a human-readable title from a workspace-relative path.
///
/// `"notes/sub/my-note.md"` → `"my-note"`
pub fn title_from_rel_path(rel_path: &str) -> String {
    rel_path
        .rsplit('/')
        .next()
        .unwrap_or(rel_path)
        .trim_end_matches(".md")
        .to_owned()
}

/// Input for [`DocumentCrud::upsert_document`]. Preserves the Tauri IPC wire
/// shape (see `src/commands/document.ts`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpsertDocumentInput {
    pub id: Option<Uuid>,
    pub workspace_id: Uuid,
    pub folder_id: Option<Uuid>,
    pub title: String,
    pub rel_path: String,
    pub file_hash: Option<String>,
}

/// Input for [`DocumentCrud::create_folder`].
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateFolderInput {
    pub workspace_id: Uuid,
    pub parent_folder_id: Option<Uuid>,
    pub name: String,
    pub rel_path: String,
}

/// Document + folder CRUD scoped to a single workspace DB.
pub struct DocumentCrud {
    db: Arc<DatabaseConnection>,
    /// Peer ID of the local device, attributed to `created_by` /
    /// `deleted_by` columns on row inserts. Supplied by AppCore.identity.
    peer_id: String,
}

impl DocumentCrud {
    pub fn new(db: Arc<DatabaseConnection>, peer_id: String) -> Self {
        Self { db, peer_id }
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    // ── Documents ──

    pub async fn list_documents(&self, workspace_id: Uuid) -> AppResult<Vec<documents::Model>> {
        Ok(documents::Entity::find()
            .filter(documents::Column::WorkspaceId.eq(workspace_id))
            .all(&*self.db)
            .await?)
    }

    pub async fn upsert_document(&self, input: UpsertDocumentInput) -> AppResult<documents::Model> {
        if let Some(id) = input.id {
            if let Some(existing) = documents::Entity::find_by_id(id).one(&*self.db).await? {
                let mut model: documents::ActiveModel = existing.into();
                model.title = Set(input.title);
                model.folder_id = Set(input.folder_id);
                model.rel_path = Set(input.rel_path);
                if let Some(hash) = input.file_hash {
                    model.file_hash = Set(Some(hash.into_bytes()));
                }
                return Ok(model.update(&*self.db).await?);
            }
        }

        let model = documents::ActiveModel {
            id: Set(input.id.unwrap_or_else(Uuid::now_v7)),
            workspace_id: Set(input.workspace_id),
            folder_id: Set(input.folder_id),
            title: Set(input.title),
            rel_path: Set(input.rel_path),
            file_hash: Set(input.file_hash.map(|h| h.into_bytes())),
            yjs_state: Set(None),
            state_vector: Set(None),
            lamport_clock: Set(0),
            created_by: Set(self.peer_id.clone()),
            ..Default::default()
        };
        Ok(model.insert(&*self.db).await?)
    }

    /// Soft-delete the document at `rel_path` by writing a tombstone to
    /// `deletion_log` and then removing the row. Idempotent — missing row
    /// is not an error.
    pub async fn delete_document_by_rel_path(&self, rel_path: &str) -> AppResult<()> {
        let Some(doc) = documents::Entity::find()
            .filter(documents::Column::RelPath.eq(rel_path))
            .one(&*self.db)
            .await?
        else {
            return Ok(());
        };

        self.write_tombstone(doc.id, rel_path.to_owned(), doc.lamport_clock + 1)
            .await?;
        documents::Entity::delete_by_id(doc.id)
            .exec(&*self.db)
            .await?;
        Ok(())
    }

    /// Cascade-delete every document whose rel_path starts with `prefix`
    /// (used when deleting a folder and its contents).
    pub async fn delete_documents_by_prefix(&self, prefix: &str) -> AppResult<u64> {
        let docs = documents::Entity::find()
            .filter(documents::Column::RelPath.starts_with(prefix))
            .all(&*self.db)
            .await?;

        let count = docs.len() as u64;
        if count == 0 {
            return Ok(0);
        }

        let now = Utc::now();
        for doc in docs {
            let doc_id = doc.id;
            let rel_path = doc.rel_path.clone();
            let lamport = doc.lamport_clock + 1;
            self.write_tombstone_at(doc_id, rel_path, lamport, now)
                .await?;
            documents::Entity::delete_by_id(doc_id)
                .exec(&*self.db)
                .await?;
        }

        tracing::info!("Cascade-deleted {count} documents under prefix '{prefix}'");
        Ok(count)
    }

    /// Rename a document: update `rel_path` + `title` on a single row by
    /// current `rel_path`. Returns the document's UUID so callers can
    /// rebase in-memory Y.Doc handles.
    pub async fn rename_document(
        &self,
        old_rel_path: &str,
        new_rel_path: String,
        new_title: String,
    ) -> AppResult<Option<Uuid>> {
        let Some(doc) = documents::Entity::find()
            .filter(documents::Column::RelPath.eq(old_rel_path))
            .one(&*self.db)
            .await?
        else {
            return Ok(None);
        };

        let doc_uuid = doc.id;
        let mut model: documents::ActiveModel = doc.into();
        model.rel_path = Set(new_rel_path);
        model.title = Set(new_title);
        model.update(&*self.db).await?;
        Ok(Some(doc_uuid))
    }

    /// Rebase every document row whose rel_path starts with `prefix_from` to
    /// the new prefix `prefix_to`. Returns the pairs `(doc_uuid, new_rel)`
    /// for caller-side Y.Doc rebasing.
    pub async fn rebase_documents_by_prefix(
        &self,
        prefix_from: &str,
        prefix_to: &str,
    ) -> AppResult<Vec<(Uuid, String)>> {
        let docs = documents::Entity::find()
            .filter(documents::Column::RelPath.starts_with(prefix_from))
            .all(&*self.db)
            .await?;

        let mut rebased = Vec::with_capacity(docs.len());
        for doc in docs {
            let new_path = format!("{prefix_to}{}", &doc.rel_path[prefix_from.len()..]);
            let doc_uuid = doc.id;
            let mut active: documents::ActiveModel = doc.into();
            active.rel_path = Set(new_path.clone());
            active.update(&*self.db).await?;
            rebased.push((doc_uuid, new_path));
        }
        Ok(rebased)
    }

    /// Update a single document row's rel_path by current rel_path. Used for
    /// file-move operations. Returns the document's UUID for caller-side
    /// Y.Doc rebasing.
    pub async fn rebase_document(&self, from_rel: &str, to_rel: String) -> AppResult<Option<Uuid>> {
        let Some(doc) = documents::Entity::find()
            .filter(documents::Column::RelPath.eq(from_rel))
            .one(&*self.db)
            .await?
        else {
            return Ok(None);
        };
        let doc_uuid = doc.id;
        let mut active: documents::ActiveModel = doc.into();
        active.rel_path = Set(to_rel);
        active.update(&*self.db).await?;
        Ok(Some(doc_uuid))
    }

    // ── Folders ──

    pub async fn list_folders(&self, workspace_id: Uuid) -> AppResult<Vec<folders::Model>> {
        Ok(folders::Entity::find()
            .filter(folders::Column::WorkspaceId.eq(workspace_id))
            .all(&*self.db)
            .await?)
    }

    pub async fn create_folder(&self, input: CreateFolderInput) -> AppResult<folders::Model> {
        let model = folders::ActiveModel {
            workspace_id: Set(input.workspace_id),
            parent_folder_id: Set(input.parent_folder_id),
            name: Set(input.name),
            rel_path: Set(input.rel_path),
            created_by: Set(self.peer_id.clone()),
            ..Default::default()
        };
        Ok(model.insert(&*self.db).await?)
    }

    /// Delete an empty folder. Fails with [`AppError::FolderNotEmpty`] if the
    /// folder contains sub-folders or documents.
    pub async fn delete_folder(&self, folder_id: Uuid) -> AppResult<()> {
        let child_folders = folders::Entity::find()
            .filter(folders::Column::ParentFolderId.eq(Some(folder_id)))
            .count(&*self.db)
            .await?;
        if child_folders > 0 {
            return Err(AppError::FolderNotEmpty("contains sub-folders".into()));
        }

        let child_docs = documents::Entity::find()
            .filter(documents::Column::FolderId.eq(Some(folder_id)))
            .count(&*self.db)
            .await?;
        if child_docs > 0 {
            return Err(AppError::FolderNotEmpty("contains documents".into()));
        }

        folders::Entity::delete_by_id(folder_id)
            .exec(&*self.db)
            .await?;
        Ok(())
    }

    // ── Reconcile (used by filesystem scan to sync newly discovered .md files) ──

    /// Insert document rows for any `.md` files on disk that aren't in the
    /// DB yet. Files in DB but missing from disk are NOT deleted (they may
    /// be a move-in-progress; cleanup is the tombstone GC's job).
    ///
    /// `disk_paths` is the set of workspace-relative `.md` paths found by
    /// scanning the filesystem (e.g. `fs::FileSystem::scan_tree` flattened).
    pub async fn reconcile_with_disk<I>(
        &self,
        workspace_id: Uuid,
        disk_paths: I,
    ) -> AppResult<usize>
    where
        I: IntoIterator<Item = String>,
    {
        let disk: std::collections::HashSet<String> = disk_paths.into_iter().collect();

        // Only fetch the `rel_path` column — full model hydration is wasteful
        // for set-difference on a 10k-doc workspace.
        let existing: std::collections::HashSet<String> = documents::Entity::find()
            .select_only()
            .column(documents::Column::RelPath)
            .filter(documents::Column::WorkspaceId.eq(workspace_id))
            .into_tuple::<String>()
            .all(&*self.db)
            .await?
            .into_iter()
            .collect();

        let missing: Vec<&String> = disk.difference(&existing).collect();
        let count = missing.len();
        if count == 0 {
            return Ok(0);
        }

        // Build all ActiveModels up front so the insert runs as a single
        // `INSERT ... VALUES (...), (...), ...` instead of N round-trips.
        // `insert_many` bypasses `ActiveModelBehavior::before_save`, so we
        // fill the timestamp fields explicitly.
        let now = Utc::now();
        let models: Vec<documents::ActiveModel> = missing
            .iter()
            .map(|rel_path| documents::ActiveModel {
                id: Set(Uuid::now_v7()),
                workspace_id: Set(workspace_id),
                folder_id: Set(None),
                title: Set(title_from_rel_path(rel_path)),
                rel_path: Set((*rel_path).clone()),
                lamport_clock: Set(0),
                created_by: Set(self.peer_id.clone()),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            })
            .collect();

        match documents::Entity::insert_many(models)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    documents::Column::WorkspaceId,
                    documents::Column::RelPath,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(&*self.db)
            .await
        {
            Ok(_) | Err(sea_orm::DbErr::RecordNotInserted) => {}
            Err(e) => {
                tracing::warn!("Failed to bulk-insert {count} document records: {e}");
                return Err(AppError::Database(e));
            }
        }

        tracing::info!("Reconcile: inserted {count} missing document records");
        Ok(count)
    }

    // ── internal ──

    async fn write_tombstone(&self, doc_id: Uuid, rel_path: String, lamport: i64) -> AppResult<()> {
        self.write_tombstone_at(doc_id, rel_path, lamport, Utc::now())
            .await
    }

    async fn write_tombstone_at(
        &self,
        doc_id: Uuid,
        rel_path: String,
        lamport: i64,
        deleted_at: chrono::DateTime<Utc>,
    ) -> AppResult<()> {
        let tombstone = deletion_log::ActiveModel {
            doc_id: Set(doc_id),
            rel_path: Set(rel_path),
            deleted_at: Set(deleted_at),
            deleted_by: Set(self.peer_id.clone()),
            lamport_clock: Set(lamport),
        };
        deletion_log::Entity::insert(tombstone)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(deletion_log::Column::DocId)
                    .update_columns([
                        deletion_log::Column::DeletedAt,
                        deletion_log::Column::DeletedBy,
                        deletion_log::Column::LamportClock,
                    ])
                    .to_owned(),
            )
            .exec(&*self.db)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_strips_md_extension() {
        assert_eq!(title_from_rel_path("note.md"), "note");
        assert_eq!(title_from_rel_path("notes/sub/diary.md"), "diary");
    }

    #[test]
    fn title_handles_no_extension() {
        assert_eq!(title_from_rel_path("folder"), "folder");
        assert_eq!(title_from_rel_path("a/b/c"), "c");
    }

    #[test]
    fn title_handles_toplevel() {
        assert_eq!(title_from_rel_path("root.md"), "root");
    }
}
