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

// ── Awareness sub-topic ──
//
// Awareness rides on a separate GossipSub topic from doc updates because:
//   1. doc-update wire format is positional `[uuid][bytes]` (no discriminator),
//      so adding a tag byte would break old binaries that read first 16 bytes
//      as a uuid.
//   2. Awareness is ephemeral — separating topics prevents accidental reuse of
//      the doc-update path's persistence/buffer infrastructure.
//   3. Old peers without awareness support never subscribe to ws-aw topics, so
//      they silently ignore awareness traffic without any warn-spam.

/// GossipSub topic format for workspace-level awareness traffic.
pub fn ws_awareness_topic(workspace_uuid: &Uuid) -> String {
    format!("swarmnote/ws-aw/{workspace_uuid}")
}

/// Parse a workspace-level awareness GossipSub topic: `swarmnote/ws-aw/{uuid}`.
pub fn parse_ws_awareness_topic(topic: &str) -> Option<Uuid> {
    topic
        .strip_prefix("swarmnote/ws-aw/")
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Encode an awareness GossipSub payload: `[16 bytes doc_uuid][awareness bytes]`.
/// Identical layout to `encode_ws_gossip` to keep the parser trivial.
pub fn encode_ws_awareness(doc_uuid: &Uuid, update: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16 + update.len());
    buf.extend_from_slice(doc_uuid.as_bytes());
    buf.extend_from_slice(update);
    buf
}

/// Decode an awareness GossipSub payload into (doc_uuid, awareness_bytes).
/// Returns `None` for empty awareness payloads.
pub fn decode_ws_awareness(data: &[u8]) -> Option<(Uuid, &[u8])> {
    if data.len() <= 16 {
        return None;
    }
    let uuid_bytes: [u8; 16] = data[..16].try_into().ok()?;
    let doc_uuid = Uuid::from_bytes(uuid_bytes);
    Some((doc_uuid, &data[16..]))
}

// ── Encrypted GossipSub payload codec (E2E) ──
//
// Wraps the legacy `[16B doc_uuid][bytes]` layout in workspace-key encryption:
//   wire = [16B doc_uuid (plaintext, routing)] [XChaCha20-Poly1305 frame]
// The doc_uuid stays plaintext so a recipient can route before decrypting, and
// is bound into the AAD so it cannot be swapped. `msg_type` separates the
// doc-update and awareness channels (prevents cross-channel replay). The frame
// header carries `key_version`, so the recipient picks the right key from its
// `{key_version → key}` history. See `dev-notes/design/08-e2e-encryption.md`.

use crate::crypto::{self, Purpose};
use crate::error::{AppError, AppResult};
use crate::workspace::keys::WorkspaceKeys;

/// AAD `msg_type` for encrypted doc-update broadcasts (`ws` channel).
pub const MSG_TYPE_DOC: u8 = 1;
/// AAD `msg_type` for encrypted awareness broadcasts (`ws-aw` channel).
pub const MSG_TYPE_AWARENESS: u8 = 2;

fn gossip_aad(workspace_id: &Uuid, doc_uuid: &Uuid, key_version: u32, msg_type: u8) -> Vec<u8> {
    let mut aad = Vec::with_capacity(16 + 16 + 4 + 1);
    aad.extend_from_slice(workspace_id.as_bytes());
    aad.extend_from_slice(doc_uuid.as_bytes());
    aad.extend_from_slice(&key_version.to_be_bytes());
    aad.push(msg_type);
    aad
}

/// Encrypt a workspace gossip payload under the current workspace read key.
pub fn encode_encrypted_gossip(
    keys: &WorkspaceKeys,
    workspace_id: &Uuid,
    doc_uuid: &Uuid,
    msg_type: u8,
    plaintext: &[u8],
) -> AppResult<Vec<u8>> {
    let (version, set) = keys.current().ok_or(AppError::Crypto {
        context: "gossip-encode",
        reason: "workspace has no key".into(),
    })?;
    let aad = gossip_aad(workspace_id, doc_uuid, version, msg_type);
    let frame = crypto::seal(
        &set.read_key,
        Purpose::Gossip,
        workspace_id.as_bytes(),
        version,
        &aad,
        plaintext,
    )?;
    let mut wire = Vec::with_capacity(16 + frame.len());
    wire.extend_from_slice(doc_uuid.as_bytes());
    wire.extend_from_slice(&frame);
    Ok(wire)
}

/// Decrypt a workspace gossip payload, selecting the key by the frame's
/// `key_version`. Returns `(doc_uuid, plaintext)`. Fails (rejects) on unknown
/// key version, commitment mismatch, AAD/channel mismatch, or tamper.
pub fn decode_encrypted_gossip(
    keys: &WorkspaceKeys,
    workspace_id: &Uuid,
    msg_type: u8,
    wire: &[u8],
) -> AppResult<(Uuid, Vec<u8>)> {
    if wire.len() <= 16 {
        return Err(AppError::Crypto {
            context: "gossip-decode",
            reason: "payload too short".into(),
        });
    }
    let uuid_bytes: [u8; 16] = wire[..16].try_into().expect("checked len > 16");
    let doc_uuid = Uuid::from_bytes(uuid_bytes);
    let frame = &wire[16..];

    let version = crypto::frame_key_version(frame)?;
    let read_key = keys.read_key(version).ok_or(AppError::Crypto {
        context: "gossip-decode",
        reason: format!("no read key for version {version}"),
    })?;
    let aad = gossip_aad(workspace_id, &doc_uuid, version, msg_type);
    let plaintext = crypto::open(
        read_key,
        Purpose::Gossip,
        workspace_id.as_bytes(),
        version,
        &aad,
        frame,
    )?;
    Ok((doc_uuid, plaintext))
}

#[cfg(test)]
mod codec_tests {
    use super::*;

    fn keys(read: u8) -> WorkspaceKeys {
        WorkspaceKeys::test_single(1, [read; 32], Some([read.wrapping_add(1); 32]))
    }

    #[test]
    fn round_trip_recovers_doc_uuid_and_plaintext() {
        let ks = keys(3);
        let ws = Uuid::from_u128(0x1111);
        let doc = Uuid::from_u128(0x2222);
        let wire = encode_encrypted_gossip(&ks, &ws, &doc, MSG_TYPE_DOC, b"y-update").unwrap();
        // doc_uuid is plaintext-routable from the wire prefix.
        assert_eq!(&wire[..16], doc.as_bytes());
        let (got_doc, pt) = decode_encrypted_gossip(&ks, &ws, MSG_TYPE_DOC, &wire).unwrap();
        assert_eq!(got_doc, doc);
        assert_eq!(pt, b"y-update");
    }

    #[test]
    fn wrong_workspace_key_rejected() {
        let ws = Uuid::from_u128(1);
        let doc = Uuid::from_u128(2);
        let wire = encode_encrypted_gossip(&keys(3), &ws, &doc, MSG_TYPE_DOC, b"x").unwrap();
        // Different read key, same version → commitment mismatch.
        assert!(decode_encrypted_gossip(&keys(9), &ws, MSG_TYPE_DOC, &wire).is_err());
    }

    #[test]
    fn cross_channel_replay_rejected() {
        let ws = Uuid::from_u128(1);
        let doc = Uuid::from_u128(2);
        let wire = encode_encrypted_gossip(&keys(3), &ws, &doc, MSG_TYPE_DOC, b"x").unwrap();
        // A doc-update frame must not decrypt as awareness (AAD msg_type differs).
        assert!(decode_encrypted_gossip(&keys(3), &ws, MSG_TYPE_AWARENESS, &wire).is_err());
    }

    #[test]
    fn unknown_key_version_rejected() {
        let ws = Uuid::from_u128(1);
        let doc = Uuid::from_u128(2);
        let wire = encode_encrypted_gossip(&keys(3), &ws, &doc, MSG_TYPE_DOC, b"x").unwrap();
        // Recipient only holds version 2 → cannot decrypt a version-1 frame.
        let only_v2 = WorkspaceKeys::test_single(2, [3; 32], Some([4; 32]));
        assert!(decode_encrypted_gossip(&only_v2, &ws, MSG_TYPE_DOC, &wire).is_err());
    }

    #[test]
    fn wrong_workspace_id_rejected() {
        let doc = Uuid::from_u128(2);
        let wire = encode_encrypted_gossip(&keys(3), &Uuid::from_u128(1), &doc, MSG_TYPE_DOC, b"x")
            .unwrap();
        // Same key but different workspace_id in AAD → reject.
        assert!(
            decode_encrypted_gossip(&keys(3), &Uuid::from_u128(99), MSG_TYPE_DOC, &wire).is_err()
        );
    }
}
