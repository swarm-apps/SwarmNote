//! Tauri IPC commands for document + folder CRUD.
//!
//! Thin wrappers over [`swarmnote_core::DocumentCrud`] (exposed via
//! [`swarmnote_core::WorkspaceCore::documents`]) and [`swarmnote_core::fs`].
//! Every command resolves its workspace through the [`WorkspaceMap`] bound
//! to the window label.

use std::path::PathBuf;

use entity::workspace::{documents, folders};
use serde::{Deserialize, Serialize};
use swarmnote_core::{AppEvent, CreateFolderInput, UpsertDocumentInput};
use tauri::{State, Window};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::platform::WorkspaceMap;

async fn workspace_from_label(
    map: &WorkspaceMap,
    label: &str,
) -> AppResult<std::sync::Arc<swarmnote_core::WorkspaceCore>> {
    map.get(label).await.ok_or(AppError::NoWorkspaceOpen)
}

#[tauri::command]
pub async fn db_get_documents(
    window: Window,
    workspace_id: Uuid,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<Vec<documents::Model>> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().list_documents(workspace_id).await
}

#[tauri::command]
pub async fn db_upsert_document(
    window: Window,
    input: UpsertDocumentInput,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<documents::Model> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().upsert_document(input).await
}

#[tauri::command]
pub async fn delete_document_by_rel_path(
    window: Window,
    rel_path: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().delete_document_by_rel_path(&rel_path).await
}

#[derive(Debug, Deserialize)]
pub struct RenameDocumentInput {
    pub old_rel_path: String,
    pub new_rel_path: String,
    pub new_title: String,
}

#[tauri::command]
pub async fn rename_document(
    window: Window,
    input: RenameDocumentInput,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    let doc_uuid = ws
        .documents()
        .rename_document(
            &input.old_rel_path,
            input.new_rel_path.clone(),
            input.new_title,
        )
        .await?;
    if let Some(uuid) = doc_uuid {
        ws.ydoc().rename_doc(uuid, &input.new_rel_path);
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_documents_by_prefix(
    window: Window,
    prefix: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<u64> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().delete_documents_by_prefix(&prefix).await
}

#[tauri::command]
pub async fn db_get_folders(
    window: Window,
    workspace_id: Uuid,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<Vec<folders::Model>> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().list_folders(workspace_id).await
}

#[tauri::command]
pub async fn db_create_folder(
    window: Window,
    input: CreateFolderInput,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<folders::Model> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().create_folder(input).await
}

#[tauri::command]
pub async fn db_delete_folder(
    window: Window,
    id: Uuid,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().delete_folder(id).await
}

// ── Move document/folder ──

#[derive(Debug, Deserialize)]
pub struct MoveDocumentInput {
    /// 源路径（文件或目录），相对工作区根。
    pub from_rel_path: String,
    /// 目标完整路径（不是目标父目录），相对工作区根。
    pub to_rel_path: String,
}

#[derive(Debug, Serialize)]
pub struct MoveDocumentResult {
    pub new_rel_path: String,
    pub is_dir: bool,
}

/// Atomically move a document or folder.
///
/// Uses [`swarmnote_core::fs::ops::move_node`] for the physical move, then
/// rebases DB rows + in-memory YDocManager entries accordingly.
#[tauri::command]
pub async fn move_document(
    window: Window,
    input: MoveDocumentInput,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<MoveDocumentResult> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    let from_rel = input.from_rel_path;
    let to_rel = input.to_rel_path;

    let move_result =
        swarmnote_core::fs::ops::move_node(ws.fs().as_ref(), &from_rel, &to_rel).await?;

    if move_result.is_dir {
        let prefix_from = if from_rel.ends_with('/') {
            from_rel.clone()
        } else {
            format!("{from_rel}/")
        };
        let prefix_to = if to_rel.ends_with('/') {
            to_rel.clone()
        } else {
            format!("{to_rel}/")
        };
        let rebased = ws
            .documents()
            .rebase_documents_by_prefix(&prefix_from, &prefix_to)
            .await?;
        for (doc_uuid, new_path) in rebased {
            ws.ydoc().rename_doc(doc_uuid, &new_path);
        }
    } else if let Some(doc_uuid) = ws
        .documents()
        .rebase_document(&from_rel, to_rel.clone())
        .await?
    {
        ws.ydoc().rename_doc(doc_uuid, &to_rel);
    }

    // Structural tree change — fire immediately so the frontend refreshes
    // without waiting for the watcher debounce.
    ws.event_bus().emit(AppEvent::FileTreeChanged {
        workspace_id: ws.info().id,
    });

    // Silence unused import when the module compiles without it — PathBuf is
    // part of the public API contract even if the current body doesn't need it.
    let _: Option<PathBuf> = None;

    Ok(MoveDocumentResult {
        new_rel_path: to_rel,
        is_dir: move_result.is_dir,
    })
}
