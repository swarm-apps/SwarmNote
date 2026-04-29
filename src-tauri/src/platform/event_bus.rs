//! `EventBus` impl that forwards `AppEvent` variants to the Tauri IPC layer
//! via `app.emit(topic, payload)` broadcasts. The frontend subscribes to the
//! specific topic names (preserved from the pre-refactor event surface) and
//! filters the payload by `doc_id` / `workspace_id` / `peer_id` as needed.

use serde_json::json;
use tauri::{AppHandle, Emitter};

use swarmnote_core::{AppEvent, EventBus};

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
        // Each arm translates a core event into one or more Tauri topic emits
        // preserving the frontend-visible event name + payload shape.
        match event {
            // ── YDoc / fs ──
            AppEvent::DocFlushed { doc_id } => {
                let _ = self
                    .app
                    .emit("yjs:flushed", json!({ "docUuid": doc_id.to_string() }));
            }
            AppEvent::ExternalUpdate { doc_id, update } => {
                let _ = self.app.emit(
                    "yjs:external-update",
                    json!({
                        "docUuid": doc_id.to_string(),
                        "update": update,
                    }),
                );
            }
            AppEvent::ExternalConflict { doc_id, rel_path } => {
                let _ = self.app.emit(
                    "yjs:external-conflict",
                    json!({
                        "docUuid": doc_id.to_string(),
                        "relPath": rel_path,
                    }),
                );
            }
            AppEvent::FileTreeChanged { workspace_id } => {
                let _ = self.app.emit(
                    "fs:tree-changed",
                    json!({ "workspaceId": workspace_id.to_string() }),
                );
            }

            // ── Devices ──
            AppEvent::DevicesChanged { devices } => {
                let _ = self.app.emit("devices-changed", devices);
            }

            // ── Pairing ──
            AppEvent::PairingRequestReceived {
                pending_id,
                peer_id,
                os_info,
                method,
                expires_at,
            } => {
                let _ = self.app.emit(
                    "pairing-request-received",
                    json!({
                        "pendingId": pending_id,
                        "peerId": peer_id,
                        "osInfo": os_info,
                        "method": method,
                        "expiresAt": expires_at,
                    }),
                );
            }
            AppEvent::PairedDeviceAdded { info } => {
                let _ = self.app.emit("paired-device-added", info);
            }
            AppEvent::PairedDeviceRemoved { peer_id } => {
                let _ = self
                    .app
                    .emit("paired-device-removed", json!({ "peerId": peer_id }));
            }

            // ── Network ──
            AppEvent::NetworkStatusChanged {
                nat_status,
                public_addr,
            } => {
                let _ = self.app.emit(
                    "network-status-changed",
                    json!({
                        "natStatus": nat_status,
                        "publicAddr": public_addr,
                    }),
                );
            }
            AppEvent::NodeStarted => {
                let _ = self.app.emit("node-started", ());
            }
            AppEvent::NodeStopped => {
                let _ = self.app.emit("node-stopped", ());
            }

            // ── Sync ──
            AppEvent::SyncStarted {
                workspace_id,
                peer_id,
            } => {
                let _ = self.app.emit(
                    "sync-started",
                    json!({
                        "workspaceUuid": workspace_id.to_string(),
                        "peerId": peer_id,
                    }),
                );
            }
            AppEvent::SyncProgress {
                workspace_id,
                peer_id,
                completed,
                total,
            } => {
                let _ = self.app.emit(
                    "sync-progress",
                    json!({
                        "workspaceUuid": workspace_id.to_string(),
                        "peerId": peer_id,
                        "completed": completed,
                        "total": total,
                    }),
                );
            }
            AppEvent::SyncCompleted {
                workspace_id,
                peer_id,
                cancelled,
            } => {
                let _ = self.app.emit(
                    "sync-completed",
                    json!({
                        "workspaceUuid": workspace_id.to_string(),
                        "peerId": peer_id,
                        "result": if cancelled { "cancelled" } else { "success" },
                    }),
                );
            }
        }
    }
}
