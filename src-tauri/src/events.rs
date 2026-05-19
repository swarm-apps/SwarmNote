//! Tauri 类型化事件
//!
//! 用 newtype + `#[serde(transparent)]` 包装 core 的 payload:wire 形状不变,
//! 同时让 tauri-specta 把 struct ident 自动转 kebab-case 作为事件名
//! (`FileTreeChanged` → `"file-tree-changed"`)。前端通过生成的
//! `events.xxx.listen()` 调用,事件名是底层细节不再裸用。

use serde::Serialize;
use swarmnote_core::protocol::{OsInfo, PairingMethod};
use swarmnote_core::{Device, PairedDeviceInfo, WorkspaceInfo};
use uuid::Uuid;

// === YDoc / 文档同步 ===

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DocFlushed {
    pub doc_uuid: Uuid,
}

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUpdate {
    pub doc_uuid: Uuid,
    pub update: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAwarenessUpdate {
    pub doc_uuid: Uuid,
    pub update: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ExternalConflict {
    pub doc_uuid: Uuid,
    pub rel_path: String,
}

// === 文件树 ===

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct FileTreeChanged {
    pub workspace_id: Uuid,
}

// === 设备 ===

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct DevicesChanged(pub Vec<Device>);

// === 配对 ===

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PairingRequestReceived {
    pub pending_id: u64,
    pub peer_id: String,
    pub os_info: OsInfo,
    pub method: PairingMethod,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// 已配对设备新增。`Option`:出站配对响应不回带 peer info(收方填),前端
/// 接到 `None` 仅作为"刷新设备列表"信号。
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct PairedDeviceAdded(pub Option<PairedDeviceInfo>);

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PairedDeviceRemoved {
    pub peer_id: String,
}

// === 网络 / P2P 节点 ===

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatusChanged {
    pub nat_status: String,
    pub public_addr: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct NodeStarted;

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct NodeStopped;

// === 工作区 sync ===

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct SyncStarted {
    pub workspace_uuid: Uuid,
    pub peer_id: String,
}

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub workspace_uuid: Uuid,
    pub peer_id: String,
    pub completed: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SyncResult {
    Success,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct SyncCompleted {
    pub workspace_uuid: Uuid,
    pub peer_id: String,
    pub result: SyncResult,
    pub error: Option<String>,
}

// === Window 内部事件 (per-window emit,不广播) ===

/// 工作区绑定到调用方窗口后的回调 payload。`open_workspace_window` 命令在创建
/// 新窗口或绑定到 caller 时,通过 `WebviewWindow::emit("workspace-ready", ...)`
/// 发送,前端用 `events.workspaceReady.listen()` 接收。
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct WorkspaceReady(pub WorkspaceInfo);

/// 设置窗口的路由切换信号 —— 用户再次点击设置入口时,把已经存在的设置窗口
/// 切到指定 sub-route(general/sync/devices/about 等)。payload 是目标路径。
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct Navigate(pub String);
