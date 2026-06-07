//! Sync sub-protocol — state-vector exchange, full-doc pulls, chunked asset
//! transfer.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRequest {
    /// Query the doc list of a specific workspace.
    DocList { workspace_uuid: Uuid },
    /// Send local state vector; ask for missing updates.
    StateVector {
        doc_id: Uuid,
        #[serde(with = "serde_bytes")]
        sv: Vec<u8>,
    },
    /// Request the full document state.
    FullSync { doc_id: Uuid },
    /// Request the manifest of a document's attached assets.
    AssetManifest { doc_id: Uuid },
    /// Request a specific chunk of an asset.
    AssetChunk {
        doc_id: Uuid,
        name: String,
        chunk_index: u32,
    },
    /// Request this workspace's symmetric key, sealed to the requester device.
    WorkspaceKey { workspace_uuid: Uuid },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    /// Document metadata list.
    DocList { docs: Vec<DocMeta> },
    /// Yjs updates the requester was missing.
    Updates {
        doc_id: Uuid,
        #[serde(with = "serde_bytes")]
        updates: Vec<u8>,
    },
    /// Asset manifest for a document.
    AssetManifest {
        doc_id: Uuid,
        assets: Vec<AssetMeta>,
    },
    /// A chunk of an asset file.
    AssetChunk {
        doc_id: Uuid,
        name: String,
        chunk_index: u32,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
        is_last: bool,
    },
    /// The workspace key sealed to the requester device (`None` if the
    /// responder has no key for that workspace or declines).
    WorkspaceKey {
        workspace_uuid: Uuid,
        sealed: Option<SealedWorkspaceKey>,
    },
}

/// A workspace key set sealed (X25519 Lockbox) to a specific recipient device.
/// Each `sealed_*` blob is a self-contained Lockbox frame. A read-only
/// recipient receives `sealed_write = None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedWorkspaceKey {
    pub key_version: u32,
    #[serde(with = "serde_bytes")]
    pub sealed_read: Vec<u8>,
    pub sealed_write: Option<Vec<u8>>,
}

/// Asset file metadata advertised via `AssetManifest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMeta {
    pub name: String,
    #[serde(with = "serde_bytes")]
    pub hash: Vec<u8>,
    pub size: u64,
}

/// Document metadata advertised via `DocList`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMeta {
    pub doc_id: Uuid,
    pub rel_path: String,
    pub title: String,
    pub updated_at: i64,
    /// `None` = active document, `Some` = deleted (tombstone).
    pub deleted_at: Option<i64>,
    /// Monotonic version clock for conflict ordering.
    pub lamport_clock: i64,
    /// Workspace UUID for cross-device workspace matching.
    pub workspace_uuid: Uuid,
}
