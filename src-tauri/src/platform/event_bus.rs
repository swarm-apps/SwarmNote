//! `EventBus` impl —— 把 `AppEvent` 翻译成 tauri-specta 类型化事件。
//!
//! 通过 [`crate::events`] 中的 newtype + `tauri_specta::Event` derive 直接
//! `XxxEvent { ... }.emit(&app)`,前端通过 `events.xxx.listen()` 接收,双方
//! 共享同一份 TS 类型(`src/lib/bindings.ts`)。

use tauri::AppHandle;
use tauri_specta::Event;

use swarmnote_core::{AppEvent, EventBus};

use crate::events::{
    DevicesChanged, DocFlushed, ExternalAwarenessUpdate, ExternalConflict, ExternalUpdate,
    FileTreeChanged, NetworkStatusChanged, NodeStarted, NodeStopped, PairedDeviceAdded,
    PairedDeviceRemoved, PairingRequestReceived, SyncCompleted, SyncProgress, SyncResult,
    SyncStarted,
};

/// `EventBus` implementation backed by Tauri's `AppHandle::emit`. Broadcasts
/// to all windows — the frontend filters by the business keys in the payload
/// rather than by a target label.
pub struct TauriEventBus {
    app: AppHandle,
}

impl TauriEventBus {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EventBus for TauriEventBus {
    fn emit(&self, event: AppEvent) {
        match event {
            // ── YDoc / fs ──
            AppEvent::DocFlushed { doc_id } => {
                let _ = DocFlushed { doc_uuid: doc_id }.emit(&self.app);
            }
            AppEvent::ExternalUpdate { doc_id, update } => {
                let _ = ExternalUpdate {
                    doc_uuid: doc_id,
                    update,
                }
                .emit(&self.app);
            }
            AppEvent::ExternalAwarenessUpdate { doc_id, update } => {
                let _ = ExternalAwarenessUpdate {
                    doc_uuid: doc_id,
                    update,
                }
                .emit(&self.app);
            }
            AppEvent::ExternalConflict { doc_id, rel_path } => {
                let _ = ExternalConflict {
                    doc_uuid: doc_id,
                    rel_path,
                }
                .emit(&self.app);
            }
            AppEvent::FileTreeChanged { workspace_id } => {
                let _ = FileTreeChanged { workspace_id }.emit(&self.app);
            }

            // ── Devices ──
            AppEvent::DevicesChanged { devices } => {
                let _ = DevicesChanged(devices).emit(&self.app);
            }

            // ── Pairing ──
            AppEvent::PairingRequestReceived {
                pending_id,
                peer_id,
                os_info,
                method,
                expires_at,
            } => {
                let _ = PairingRequestReceived {
                    pending_id,
                    peer_id,
                    os_info,
                    method,
                    expires_at,
                }
                .emit(&self.app);
            }
            AppEvent::PairedDeviceAdded { info } => {
                let _ = PairedDeviceAdded(info).emit(&self.app);
            }
            AppEvent::PairedDeviceRemoved { peer_id } => {
                let _ = PairedDeviceRemoved { peer_id }.emit(&self.app);
            }

            // ── Network ──
            AppEvent::NetworkStatusChanged {
                nat_status,
                public_addr,
            } => {
                let _ = NetworkStatusChanged {
                    nat_status,
                    public_addr,
                }
                .emit(&self.app);
            }
            AppEvent::NodeStarted => {
                let _ = NodeStarted.emit(&self.app);
            }
            AppEvent::NodeStopped => {
                let _ = NodeStopped.emit(&self.app);
            }

            // ── Sync ──
            AppEvent::SyncStarted {
                workspace_id,
                peer_id,
            } => {
                let _ = SyncStarted {
                    workspace_uuid: workspace_id,
                    peer_id,
                }
                .emit(&self.app);
            }
            AppEvent::SyncProgress {
                workspace_id,
                peer_id,
                completed,
                total,
            } => {
                let _ = SyncProgress {
                    workspace_uuid: workspace_id,
                    peer_id,
                    completed,
                    total,
                }
                .emit(&self.app);
            }
            AppEvent::SyncCompleted {
                workspace_id,
                peer_id,
                cancelled,
                error,
            } => {
                let result = if cancelled {
                    SyncResult::Cancelled
                } else if error.is_some() {
                    SyncResult::Error
                } else {
                    SyncResult::Success
                };
                let _ = SyncCompleted {
                    workspace_uuid: workspace_id,
                    peer_id,
                    result,
                    error,
                }
                .emit(&self.app);
            }
        }
    }
}
