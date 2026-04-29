//! P2P wire protocol — top-level `AppRequest` / `AppResponse` plus three
//! sub-protocols (pairing, workspace, sync), exchanged over `swarm-p2p-core`'s
//! typed request-response channels.

pub mod os_info;
pub mod pairing;
pub mod sync;
pub mod workspace;

use serde::{Deserialize, Serialize};

pub use os_info::OsInfo;
pub use pairing::{PairingMethod, PairingRefuseReason, PairingRequest, PairingResponse};
pub use sync::{AssetMeta, DocMeta, SyncRequest, SyncResponse};
pub use workspace::{WorkspaceMeta, WorkspaceRequest, WorkspaceResponse};

/// Top-level request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppRequest {
    Pairing(PairingRequest),
    Workspace(WorkspaceRequest),
    Sync(SyncRequest),
}

/// Top-level response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppResponse {
    Pairing(PairingResponse),
    Workspace(WorkspaceResponse),
    Sync(SyncResponse),
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn app_request_cbor_roundtrip() {
        let requests = vec![
            AppRequest::Sync(SyncRequest::DocList {
                workspace_uuid: Uuid::now_v7(),
            }),
            AppRequest::Sync(SyncRequest::FullSync {
                doc_id: Uuid::now_v7(),
            }),
            AppRequest::Sync(SyncRequest::StateVector {
                doc_id: Uuid::now_v7(),
                sv: vec![1, 2, 3, 4],
            }),
            AppRequest::Pairing(PairingRequest {
                os_info: OsInfo::default(),
                timestamp: 1234567890,
                method: PairingMethod::Code {
                    code: "123456".to_string(),
                },
            }),
            AppRequest::Pairing(PairingRequest {
                os_info: OsInfo::default(),
                timestamp: 1234567890,
                method: PairingMethod::Direct,
            }),
            AppRequest::Sync(SyncRequest::AssetManifest {
                doc_id: Uuid::now_v7(),
            }),
            AppRequest::Sync(SyncRequest::AssetChunk {
                doc_id: Uuid::now_v7(),
                name: "screenshot-af3b9e2c.png".to_string(),
                chunk_index: 0,
            }),
            AppRequest::Workspace(WorkspaceRequest::ListWorkspaces),
        ];

        for req in requests {
            let json = serde_json::to_string(&req).unwrap();
            let restored: AppRequest = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&restored).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn app_response_cbor_roundtrip() {
        let responses = vec![
            AppResponse::Sync(SyncResponse::DocList {
                docs: vec![DocMeta {
                    doc_id: Uuid::now_v7(),
                    rel_path: "notes/todo.md".to_string(),
                    title: "Test Note".to_string(),
                    updated_at: 1234567890,
                    deleted_at: None,
                    lamport_clock: 0,
                    workspace_uuid: Uuid::now_v7(),
                }],
            }),
            AppResponse::Sync(SyncResponse::Updates {
                doc_id: Uuid::now_v7(),
                updates: vec![10, 20, 30],
            }),
            AppResponse::Sync(SyncResponse::AssetManifest {
                doc_id: Uuid::now_v7(),
                assets: vec![AssetMeta {
                    name: "screenshot-af3b9e2c.png".to_string(),
                    hash: vec![1, 2, 3, 4],
                    size: 256000,
                }],
            }),
            AppResponse::Sync(SyncResponse::AssetChunk {
                doc_id: Uuid::now_v7(),
                name: "screenshot-af3b9e2c.png".to_string(),
                chunk_index: 0,
                data: vec![0u8; 128],
                is_last: true,
            }),
            AppResponse::Pairing(PairingResponse::Success),
            AppResponse::Pairing(PairingResponse::Refused {
                reason: PairingRefuseReason::CodeExpired,
            }),
            AppResponse::Workspace(WorkspaceResponse::WorkspaceList {
                workspaces: vec![WorkspaceMeta {
                    uuid: Uuid::now_v7(),
                    name: "Test Workspace".to_string(),
                    doc_count: 42,
                    updated_at: 1234567890,
                }],
            }),
        ];

        for resp in responses {
            let json = serde_json::to_string(&resp).unwrap();
            let restored: AppResponse = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&restored).unwrap();
            assert_eq!(json, json2);
        }
    }
}
