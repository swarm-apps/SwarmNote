//! Tauri IPC commands for P2P node lifecycle.
//!
//! Thin wrappers over [`swarmnote_core::AppCore`] network lifecycle methods.
//! All business logic (swarm init, DHT bootstrap, event loop) lives in core;
//! these commands only expose the methods to the frontend and handle
//! tray-integration side effects.

use std::sync::Arc;

use swarmnote_core::{AppCore, Device, DeviceFilter, NodeStatus};
use tauri::{AppHandle, Manager, State};

use crate::error::AppResult;

/// 启动 P2P 节点。
#[tauri::command]
pub async fn start_p2p_node(app: AppHandle, core: State<'_, Arc<AppCore>>) -> AppResult<()> {
    let core = core.inner().clone();
    core.start_network().await?;

    // Tray status side-effect (desktop only).
    #[cfg(desktop)]
    if let Some(tray) = app.try_state::<crate::tray::TrayManagerState>() {
        tray.lock()
            .await
            .set_status(crate::tray::TrayNodeStatus::Running { peer_count: 0 })
            .await;
    }
    let _ = app;
    Ok(())
}

/// 停止 P2P 节点。
#[tauri::command]
pub async fn stop_p2p_node(app: AppHandle, core: State<'_, Arc<AppCore>>) -> AppResult<()> {
    core.stop_network().await?;

    #[cfg(desktop)]
    if let Some(tray) = app.try_state::<crate::tray::TrayManagerState>() {
        tray.lock()
            .await
            .set_status(crate::tray::TrayNodeStatus::Stopped)
            .await;
    }
    let _ = app;
    Ok(())
}

/// 查询 P2P 节点当前运行状态。
#[tauri::command]
pub async fn get_network_status(core: State<'_, Arc<AppCore>>) -> AppResult<NodeStatus> {
    Ok(core.network_status().await)
}

/// 获取已连接的设备列表。
#[tauri::command]
pub async fn get_connected_peers(core: State<'_, Arc<AppCore>>) -> AppResult<Vec<Device>> {
    match core.devices().await {
        Ok(dm) => Ok(dm.get_devices(DeviceFilter::Connected)),
        Err(_) => Ok(Vec::new()),
    }
}
