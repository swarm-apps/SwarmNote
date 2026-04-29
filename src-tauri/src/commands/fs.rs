//! Tauri IPC commands for filesystem operations inside a workspace.
//!
//! Thin wrappers over [`swarmnote_core::FileSystem`] and [`swarmnote_core::fs::ops`].
//! Each command resolves its workspace via [`WorkspaceMap`] (window label → core).

use std::sync::Arc;

use serde::Serialize;
use swarmnote_core::{FileTreeNode, WorkspaceCore};
use tauri::{State, Window};

use crate::error::{AppError, AppResult};
use crate::platform::WorkspaceMap;

#[derive(Debug, Serialize)]
pub struct SaveDocumentResult {
    /// blake3 hash hex string
    pub file_hash: String,
}

async fn workspace_from_label(map: &WorkspaceMap, label: &str) -> AppResult<Arc<WorkspaceCore>> {
    map.get(label).await.ok_or(AppError::NoWorkspaceOpen)
}

#[tauri::command]
pub async fn load_document(
    window: Window,
    rel_path: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<String> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    match ws.fs().read_text(&rel_path).await {
        Ok(s) => Ok(s),
        Err(AppError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn save_document(
    window: Window,
    rel_path: String,
    content: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<SaveDocumentResult> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.fs().write_text(&rel_path, &content).await?;
    let hash = blake3::hash(content.as_bytes());
    Ok(SaveDocumentResult {
        file_hash: hash.to_hex().to_string(),
    })
}

#[tauri::command]
pub async fn save_media(
    window: Window,
    rel_path: String,
    file_name: String,
    data: Vec<u8>,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<String> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.fs().save_media(&rel_path, &file_name, &data).await
}

#[tauri::command]
pub async fn scan_workspace_tree(
    window: Window,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<Vec<FileTreeNode>> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.fs().scan_tree("").await
}

#[tauri::command]
pub async fn fs_create_file(
    window: Window,
    parent_rel: String,
    name: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<String> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    swarmnote_core::fs::ops::create_file(ws.fs().as_ref(), &parent_rel, &name).await
}

#[tauri::command]
pub async fn fs_create_dir(
    window: Window,
    parent_rel: String,
    name: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<String> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    swarmnote_core::fs::ops::create_dir(ws.fs().as_ref(), &parent_rel, &name).await
}

#[tauri::command]
pub async fn fs_delete_file(
    window: Window,
    rel_path: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    swarmnote_core::fs::ops::delete_file(ws.fs().as_ref(), &rel_path).await
}

#[tauri::command]
pub async fn fs_delete_dir(
    window: Window,
    rel_path: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    swarmnote_core::fs::ops::delete_dir(ws.fs().as_ref(), &rel_path).await
}

#[tauri::command]
pub async fn fs_rename(
    window: Window,
    rel_path: String,
    new_name: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<String> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    swarmnote_core::fs::ops::rename(ws.fs().as_ref(), &rel_path, &new_name).await
}
