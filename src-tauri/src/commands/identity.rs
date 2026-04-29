//! Tauri IPC commands for device identity / name management.

use std::sync::Arc;

use swarmnote_core::{AppCore, DeviceInfo};
use tauri::State;
use tracing::info;

use crate::error::AppResult;

/// Return current device info.
#[tauri::command]
pub fn get_device_info(core: State<'_, Arc<AppCore>>) -> AppResult<DeviceInfo> {
    core.identity().device_info()
}

/// Update device name and persist to config; restart P2P node if running
/// so the new name propagates via libp2p Identify agent_version.
#[tauri::command]
pub async fn set_device_name(name: String, core: State<'_, Arc<AppCore>>) -> AppResult<()> {
    // In-memory identity snapshot.
    core.identity().set_device_name(name.clone())?;

    // Persist to config on disk.
    {
        let mut cfg = core.config().write().await;
        cfg.device_name = name.clone();
        swarmnote_core::config::save_config(core.config().path(), &cfg)?;
    }

    // Restart P2P if it was running so the Identify agent_version updates.
    let was_running = core.net().await.is_some();
    if was_running {
        info!("Restarting P2P node to propagate new device name: {name}");
        core.stop_network().await?;
        let core_arc = core.inner().clone();
        core_arc.start_network().await?;
    }

    Ok(())
}
