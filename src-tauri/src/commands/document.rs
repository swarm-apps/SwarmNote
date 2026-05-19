//! Tauri IPC commands for document + folder CRUD.
//!
//! Thin wrappers over [`swarmnote_core::DocumentCrud`] (exposed via
//! [`swarmnote_core::WorkspaceCore::documents`]) and [`swarmnote_core::fs`].
//! Every command resolves its workspace through the [`WorkspaceMap`] bound
//! to the window label.

use chrono::{DateTime, Utc};
use entity::workspace::folders;
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

/// 文件夹行 —— `db_create_folder` / `db_get_folders` 的 IPC 返回类型。
///
/// `entity::folders::Model` 的 struct 名硬编码为 `Model`,与 `documents::Model`
/// 撞名后会在 TS bindings 里冲突。这里定义投影 DTO 解耦 sea-orm relation 字段。
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_folder_id: Option<Uuid>,
    pub name: String,
    pub rel_path: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<folders::Model> for FolderRow {
    fn from(m: folders::Model) -> Self {
        Self {
            id: m.id,
            workspace_id: m.workspace_id,
            parent_folder_id: m.parent_folder_id,
            name: m.name,
            rel_path: m.rel_path,
            created_by: m.created_by,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn db_upsert_document(
    window: Window,
    input: UpsertDocumentInput,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().upsert_document(input).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_document_by_rel_path(
    window: Window,
    rel_path: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().delete_document_by_rel_path(&rel_path).await
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RenameDocumentInput {
    pub old_rel_path: String,
    pub new_rel_path: String,
    pub new_title: String,
}

#[tauri::command]
#[specta::specta]
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
#[specta::specta]
pub async fn delete_documents_by_prefix(
    window: Window,
    prefix: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<u64> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().delete_documents_by_prefix(&prefix).await
}

#[tauri::command]
#[specta::specta]
pub async fn db_get_folders(
    window: Window,
    workspace_id: Uuid,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<Vec<FolderRow>> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    let rows = ws.documents().list_folders(workspace_id).await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn db_create_folder(
    window: Window,
    input: CreateFolderInput,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<FolderRow> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    Ok(ws.documents().create_folder(input).await?.into())
}

#[tauri::command]
#[specta::specta]
pub async fn db_delete_folder(
    window: Window,
    id: Uuid,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.documents().delete_folder(id).await
}

// ── Move document/folder ──

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MoveDocumentInput {
    /// 源路径（文件或目录），相对工作区根。
    pub from_rel_path: String,
    /// 目标完整路径（不是目标父目录），相对工作区根。
    pub to_rel_path: String,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MoveDocumentResult {
    pub new_rel_path: String,
    pub is_dir: bool,
}

/// Atomically move a document or folder.
///
/// Uses [`swarmnote_core::fs::ops::move_node`] for the physical move, then
/// rebases DB rows + in-memory YDocManager entries accordingly.
#[tauri::command]
#[specta::specta]
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

    Ok(MoveDocumentResult {
        new_rel_path: to_rel,
        is_dir: move_result.is_dir,
    })
}
