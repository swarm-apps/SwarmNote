//! Workspace sub-protocol — resource discovery between peers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 工作区资源发现 / 分享请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceRequest {
    /// 查询对端当前打开的工作区列表。
    ListWorkspaces,
    /// 邀请本设备协作某工作区(owner 发起)。对端弹窗让用户接受/拒绝,
    /// 响应回 [`WorkspaceResponse::ShareInvitationResult`]。在对端接受前
    /// owner 不会签发 grant op,即未接受不授权。
    ShareInvitation {
        workspace_uuid: Uuid,
        /// 工作区名,仅用于在邀请弹窗里展示。
        name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceResponse {
    WorkspaceList {
        workspaces: Vec<WorkspaceMeta>,
    },
    /// 被邀请方对 [`WorkspaceRequest::ShareInvitation`] 的应答。
    ShareInvitationResult {
        accepted: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub uuid: Uuid,
    pub name: String,
    pub doc_count: u32,
    pub updated_at: i64,
}
