//! Platform-abstracted event push channel.
//!
//! Replaces direct `tauri::AppHandle::emit()` calls inside the core layer.
//! Every event the core needs to emit goes through `EventBus::emit(AppEvent)`;
//! each host implementation pattern-matches the `AppEvent` enum and translates
//! it into its native form (Tauri IPC topic, uniffi callback, etc.).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::device::Device;
use crate::pairing::PairedDeviceInfo;
use crate::protocol::{OsInfo, PairingMethod};

/// Non-blocking event sink. `emit` MUST NOT acquire any core-layer locks —
/// implementations may hop onto another thread if they need to.
pub trait EventBus: Send + Sync + 'static {
    fn emit(&self, event: AppEvent);
}

/// All events produced by the core layer. Variants carry business keys
/// (`workspace_id`, `doc_id`, `peer_id`) but never platform-specific fields —
/// host implementations route/filter based on the key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AppEvent {
    // ── YDoc / documents ──
    /// Document has been flushed to disk + DB (writeback task).
    DocFlushed {
        doc_id: Uuid,
    },
    /// Remote update applied to an open document — the editor for that doc
    /// should refresh from the supplied update bytes.
    ExternalUpdate {
        doc_id: Uuid,
        #[serde(with = "serde_bytes")]
        update: Vec<u8>,
    },
    /// Remote awareness (caret / presence) update for an open document.
    /// Bytes are an opaque `y-protocols/awareness` encodeAwarenessUpdate
    /// payload — core never decodes them. Frontend SHOULD apply via
    /// `applyAwarenessUpdate(awareness, bytes, 'remote-awareness')`.
    /// MUST NOT be persisted — awareness is ephemeral by design.
    ExternalAwarenessUpdate {
        doc_id: Uuid,
        #[serde(with = "serde_bytes")]
        update: Vec<u8>,
    },
    /// An external editor modified a `.md` file while the user had unsaved
    /// edits — frontend MUST prompt for reload/keep.
    ExternalConflict {
        doc_id: Uuid,
        rel_path: String,
    },

    // ── File tree ──
    /// Workspace file tree has changed (external editor created / deleted /
    /// moved a file). Frontend should re-scan.
    FileTreeChanged {
        workspace_id: Uuid,
    },

    // ── Devices / discovery ──
    /// Full device list snapshot — front-end replaces its table atomically.
    DevicesChanged {
        devices: Vec<Device>,
    },

    // ── Pairing ──
    /// Inbound pairing request awaiting user confirmation.
    PairingRequestReceived {
        pending_id: u64,
        peer_id: String,
        os_info: OsInfo,
        method: PairingMethod,
        expires_at: DateTime<Utc>,
    },
    /// A device was successfully paired (outbound or inbound). `info` is
    /// `None` for outbound pairing responses that don't echo the peer info.
    PairedDeviceAdded {
        info: Option<PairedDeviceInfo>,
    },
    PairedDeviceRemoved {
        peer_id: String,
    },

    // ── Sharing / membership ──
    /// 收到一条工作区协作邀请,等待用户接受/拒绝。前端 SHOULD 弹窗,用户决定后
    /// 调 `respond_share_invitation(pending_id, accept)`。邀请方的请求在此期间
    /// 阻塞等待(经 request-response 回填)。
    ShareInvitationReceived {
        pending_id: u64,
        peer_id: String,
        workspace_uuid: Uuid,
        workspace_name: String,
        expires_at: DateTime<Utc>,
    },
    /// This device was removed from a shared workspace (its role was revoked by
    /// the owner). The device has stopped subscribing to that workspace's
    /// realtime updates; frontend SHOULD notify the user. Content already
    /// synced stays locally readable.
    MemberRevoked {
        workspace_id: Uuid,
    },

    // ── Network / P2P node ──
    /// NAT status changed (behind symmetric NAT, public reachable, etc.).
    NetworkStatusChanged {
        nat_status: String,
        public_addr: Option<String>,
    },
    NodeStarted,
    NodeStopped,

    // ── Sync (per-peer, per-workspace session) ──
    SyncStarted {
        workspace_id: Uuid,
        peer_id: String,
    },
    SyncProgress {
        workspace_id: Uuid,
        peer_id: String,
        completed: u32,
        total: u32,
    },
    SyncCompleted {
        workspace_id: Uuid,
        peer_id: String,
        /// `true` if the session was cancelled mid-run; `false` = normal finish.
        cancelled: bool,
        /// `Some` if the session terminated early due to an internal error
        /// (e.g. `request_doc_list` timeout). `None` for clean finish or
        /// user-cancelled. Frontend can show this as a sync failure reason.
        error: Option<String>,
    },
}
