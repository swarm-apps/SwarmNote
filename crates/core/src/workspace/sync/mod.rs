pub(crate) mod asset_sync;
pub mod coordinator;
mod doc_sync;
mod full_sync;
mod pending_buffer;
pub mod workspace_sync;

pub use coordinator::AppSyncCoordinator;
pub use workspace_sync::WorkspaceSync;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Per-document sync status, emitted to frontend via Tauri events.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocSyncStatus {
    /// Fully synced with all connected peers.
    Synced,
    /// Currently receiving/sending updates.
    Syncing,
    /// Queued for sync (full sync hasn't reached this doc yet).
    Pending,
    /// Local-only modifications, no peers connected.
    LocalOnly,
}

// ── GossipSub control topic ──

/// Global control topic for broadcasting workspace state changes.
pub const CTRL_TOPIC: &str = "swarmnote/ctrl";

/// Control-plane messages broadcast via GossipSub (not request-response).
#[derive(Debug, Serialize, Deserialize)]
pub enum CtrlMessage {
    /// A peer opened a workspace — receivers with the same workspace should subscribe + sync.
    WorkspaceOpened { uuid: Uuid },
}

pub fn encode_ctrl_message(msg: &CtrlMessage) -> Vec<u8> {
    serde_json::to_vec(msg).expect("CtrlMessage serialization cannot fail")
}

pub fn decode_ctrl_message(data: &[u8]) -> Option<CtrlMessage> {
    serde_json::from_slice(data).ok()
}

// ── GossipSub workspace topic payload encoding ──

/// GossipSub topic format for workspace-level document updates.
pub fn ws_topic(workspace_uuid: &Uuid) -> String {
    format!("swarmnote/ws/{workspace_uuid}")
}

/// Encode a workspace GossipSub payload: `[16 bytes doc_uuid][update bytes]`
pub fn encode_ws_gossip(doc_uuid: &Uuid, update: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16 + update.len());
    buf.extend_from_slice(doc_uuid.as_bytes());
    buf.extend_from_slice(update);
    buf
}

/// Decode a workspace GossipSub payload into (doc_uuid, update_bytes).
/// Rejects payloads with no actual update content (must be > 16 bytes).
pub fn decode_ws_gossip(data: &[u8]) -> Option<(Uuid, &[u8])> {
    if data.len() <= 16 {
        return None;
    }
    let uuid_bytes: [u8; 16] = data[..16].try_into().ok()?;
    let doc_uuid = Uuid::from_bytes(uuid_bytes);
    Some((doc_uuid, &data[16..]))
}

// ── Topic parsers (used by event_loop) ──

/// Parse a legacy per-doc GossipSub topic: `swarmnote/doc/{uuid}`.
pub fn parse_sync_topic(topic: &str) -> Option<Uuid> {
    topic
        .strip_prefix("swarmnote/doc/")
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Parse a workspace-level GossipSub topic: `swarmnote/ws/{uuid}`.
pub fn parse_ws_topic(topic: &str) -> Option<Uuid> {
    topic
        .strip_prefix("swarmnote/ws/")
        .and_then(|s| Uuid::parse_str(s).ok())
}
