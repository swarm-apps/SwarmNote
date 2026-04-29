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
    },
}
