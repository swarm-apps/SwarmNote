//! Tauri IPC commands for Y.Doc management.
//!
//! Thin wrappers over [`swarmnote_core::YDocManager`] (obtained via
//! [`swarmnote_core::WorkspaceCore::ydoc`]). The `workspace_id` argument on
//! `open_ydoc` / `hydrate_workspace` disambiguates which workspace to address
//! independently of the window label.

use std::sync::Arc;

use swarmnote_core::{AppCore, OpenDocResult, WorkspaceCore};
use swarmnote_core::{HydrateProgress, HydrateProgressFn, HydrateResult};
use tauri::ipc::Channel;
use tauri::{State, Window};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::platform::WorkspaceMap;

async fn workspace_from_label(map: &WorkspaceMap, label: &str) -> AppResult<Arc<WorkspaceCore>> {
    map.get(label).await.ok_or(AppError::NoWorkspaceOpen)
}

fn parse_doc_uuid(doc_uuid: &str) -> AppResult<Uuid> {
    doc_uuid
        .parse()
        .map_err(|e| AppError::InvalidPath(format!("invalid doc_uuid: {e}")))
}

#[tauri::command]
pub async fn open_ydoc(
    window: Window,
    rel_path: String,
    _workspace_id: Uuid,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<OpenDocResult> {
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.ydoc().open_doc(&rel_path).await
}

#[tauri::command]
pub async fn apply_ydoc_update(
    window: Window,
    doc_uuid: String,
    update: Vec<u8>,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let uuid = parse_doc_uuid(&doc_uuid)?;
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.ydoc().apply_update(uuid, &update).await?;

    // Broadcast local edit to the workspace GossipSub topic (best-effort).
    // Safe from loops: local edit only — remote updates arrive via the
    // event_loop's GossipSub handler, which routes to a different path.
    if let Some(ws_sync) = ws.sync().await {
        ws_sync.publish_doc_update(uuid, update).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn close_ydoc(
    window: Window,
    doc_uuid: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let uuid = parse_doc_uuid(&doc_uuid)?;
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.ydoc().close_doc(uuid).await
}

#[tauri::command]
pub async fn rename_ydoc(
    window: Window,
    doc_uuid: String,
    new_rel_path: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let uuid = parse_doc_uuid(&doc_uuid)?;
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.ydoc().rename_doc(uuid, &new_rel_path);
    Ok(())
}

#[tauri::command]
pub async fn reload_ydoc_confirmed(
    window: Window,
    doc_uuid: String,
    ws_map: State<'_, WorkspaceMap>,
) -> AppResult<()> {
    let uuid = parse_doc_uuid(&doc_uuid)?;
    let ws = workspace_from_label(&ws_map, window.label()).await?;
    ws.ydoc().reload_confirmed(uuid).await
}

#[tauri::command]
pub async fn hydrate_workspace(
    workspace_uuid: Uuid,
    on_progress: Channel<HydrateProgress>,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<HydrateResult> {
    let ws = core
        .get_workspace(&workspace_uuid)
        .await
        .ok_or(AppError::NoWorkspaceOpen)?;

    let progress: HydrateProgressFn = {
        let ch = on_progress.clone();
        Arc::new(move |p: HydrateProgress| {
            let _ = ch.send(p);
        })
    };

    swarmnote_core::hydrate_workspace(ws.db(), ws.fs().as_ref(), workspace_uuid, &progress).await
}
