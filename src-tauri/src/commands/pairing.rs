//! Tauri IPC commands for device pairing.
//!
//! Thin wrappers over [`swarmnote_core::PairingManager`] obtained through
//! [`swarmnote_core::AppCore`]. All DHT / request-response logic lives in
//! core; these commands only translate IPC payloads and emit device-list
//! refresh events.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use swarmnote_core::{
    AppCore, AppEvent, Device, DeviceFilter, DeviceListResult, DeviceStatus, PairedDeviceInfo,
    PairingCodeInfo,
};
use tauri::State;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::error::AppResult;

use swarmnote_core::protocol::{
    AppRequest, AppResponse, OsInfo, PairingMethod, PairingResponse, WorkspaceRequest,
    WorkspaceResponse,
};

/// `get_device_by_code` 的类型化返回值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceByCodeResult {
    pub peer_id: String,
    pub os_info: OsInfo,
}

// ── 配对命令 ──

#[tauri::command]
pub async fn generate_pairing_code(
    core: State<'_, Arc<AppCore>>,
    expires_in_secs: Option<u64>,
) -> AppResult<PairingCodeInfo> {
    core.pairing()
        .await?
        .generate_code(expires_in_secs.unwrap_or(300))
        .await
}

#[tauri::command]
pub async fn get_device_by_code(
    core: State<'_, Arc<AppCore>>,
    code: String,
) -> AppResult<DeviceByCodeResult> {
    let (peer_id, record) = core.pairing().await?.get_device_by_code(&code).await?;
    Ok(DeviceByCodeResult {
        peer_id,
        os_info: record.os_info,
    })
}

#[tauri::command]
pub async fn request_pairing(
    core: State<'_, Arc<AppCore>>,
    peer_id: String,
    method: PairingMethod,
    remote_os_info: Option<OsInfo>,
) -> AppResult<PairingResponse> {
    let resp = core
        .pairing()
        .await?
        .request_pairing(&peer_id, method, remote_os_info)
        .await?;

    if matches!(resp, PairingResponse::Success) {
        core.event_bus()
            .emit(AppEvent::PairedDeviceAdded { info: None });
        emit_devices(&core).await;
    }

    Ok(resp)
}

#[tauri::command]
pub async fn respond_pairing_request(
    core: State<'_, Arc<AppCore>>,
    pending_id: u64,
    accept: bool,
) -> AppResult<()> {
    let result = core
        .pairing()
        .await?
        .handle_pairing_request(pending_id, accept)
        .await?;
    if let Some(info) = result {
        core.event_bus()
            .emit(AppEvent::PairedDeviceAdded { info: Some(info) });
        emit_devices(&core).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_paired_devices(core: State<'_, Arc<AppCore>>) -> AppResult<Vec<PairedDeviceInfo>> {
    match core.pairing().await {
        Ok(pairing) => Ok(pairing.get_paired_devices()),
        Err(_) => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn unpair_device(core: State<'_, Arc<AppCore>>, peer_id: String) -> AppResult<()> {
    core.pairing().await?.unpair(&peer_id).await?;
    core.event_bus()
        .emit(AppEvent::PairedDeviceRemoved { peer_id });
    emit_devices(&core).await;
    Ok(())
}

// ── 设备查询命令 ──

#[tauri::command]
pub async fn list_devices(
    core: State<'_, Arc<AppCore>>,
    filter: Option<DeviceFilter>,
) -> AppResult<DeviceListResult> {
    let dm = core.devices().await?;
    let devices = dm.get_devices(filter.unwrap_or_default());
    let total = devices.len();
    Ok(DeviceListResult { devices, total })
}

#[tauri::command]
pub async fn get_nearby_devices(core: State<'_, Arc<AppCore>>) -> AppResult<Vec<Device>> {
    let dm = match core.devices().await {
        Ok(dm) => dm,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(dm
        .get_devices(DeviceFilter::Connected)
        .into_iter()
        .filter(|d| !d.is_paired)
        .collect())
}

// ── 工作区列表交换 ──

/// 远程工作区信息（合并来源 peer 信息）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceInfo {
    pub uuid: Uuid,
    pub name: String,
    pub doc_count: u32,
    pub updated_at: i64,
    pub peer_id: String,
    pub peer_name: String,
    pub is_local: bool,
}

/// 并发查询所有已配对在线 peer 的工作区列表，标记 is_local。
#[tauri::command]
pub async fn get_remote_workspaces(
    core: State<'_, Arc<AppCore>>,
) -> AppResult<Vec<RemoteWorkspaceInfo>> {
    let client = core.client().await?;
    let dm = core.devices().await?;

    let paired_online: Vec<Device> = dm
        .get_devices(DeviceFilter::Paired)
        .into_iter()
        .filter(|d| d.status == DeviceStatus::Online)
        .collect();

    if paired_online.is_empty() {
        return Ok(Vec::new());
    }

    // 并发向所有 peer 发送 ListWorkspaces，5s 超时
    let mut tasks = JoinSet::new();
    for device in &paired_online {
        let peer_id_str = device.peer_id.clone();
        let peer_name = device.hostname.clone();
        let client = client.clone();
        tasks.spawn(async move {
            let Ok(peer_id) = peer_id_str.parse::<swarm_p2p_core::libp2p::PeerId>() else {
                return (peer_id_str, peer_name, None);
            };
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                client.send_request(
                    peer_id,
                    AppRequest::Workspace(WorkspaceRequest::ListWorkspaces),
                ),
            )
            .await;
            let workspaces = match result {
                Ok(Ok(AppResponse::Workspace(WorkspaceResponse::WorkspaceList { workspaces }))) => {
                    Some(workspaces)
                }
                _ => None,
            };
            (peer_id_str, peer_name, workspaces)
        });
    }

    // Local UUIDs come from AppCore's active workspace registry.
    let local_uuids: std::collections::HashSet<Uuid> = core
        .list_workspaces()
        .await
        .into_iter()
        .map(|w| w.info().id)
        .collect();

    let mut results = Vec::new();
    while let Some(Ok((peer_id, peer_name, Some(workspaces)))) = tasks.join_next().await {
        for ws in workspaces {
            results.push(RemoteWorkspaceInfo {
                is_local: local_uuids.contains(&ws.uuid),
                uuid: ws.uuid,
                name: ws.name,
                doc_count: ws.doc_count,
                updated_at: ws.updated_at,
                peer_id: peer_id.clone(),
                peer_name: peer_name.clone(),
            });
        }
    }

    Ok(results)
}

// ── Helpers ──

/// 配对变更后 emit 完整设备列表，确保前端即时更新。
async fn emit_devices(core: &Arc<AppCore>) {
    if let Ok(dm) = core.devices().await {
        let devices = dm.get_devices(DeviceFilter::All);
        core.event_bus().emit(AppEvent::DevicesChanged { devices });
    }
}
