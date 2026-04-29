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
