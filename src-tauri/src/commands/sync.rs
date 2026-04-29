//! Tauri IPC commands for explicit per-workspace sync triggers.

use std::sync::Arc;

use swarm_p2p_core::libp2p::PeerId;
use swarmnote_core::AppCore;
use tauri::State;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[tauri::command]
pub async fn trigger_workspace_sync(
    workspace_uuid: String,
    peer_id: String,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<()> {
    let coordinator = core.sync_coordinator_or_err().await?;
    let uuid = Uuid::parse_str(&workspace_uuid)
        .map_err(|e| AppError::InvalidPath(format!("Invalid UUID: {e}")))?;
    let pid: PeerId = peer_id
        .parse()
        .map_err(|e| AppError::InvalidPath(format!("Invalid PeerId: {e}")))?;
    coordinator.spawn_full_sync(pid, uuid).await;
    Ok(())
}
