//! Workspace sub-protocol — resource discovery between peers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Workspace resource discovery requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceRequest {
    /// Query the peer's currently-open workspace list.
    ListWorkspaces,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceResponse {
    WorkspaceList { workspaces: Vec<WorkspaceMeta> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub uuid: Uuid,
    pub name: String,
    pub doc_count: u32,
    pub updated_at: i64,
}
