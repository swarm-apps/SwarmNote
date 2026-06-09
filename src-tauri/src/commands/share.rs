//! Tauri IPC commands for workspace sharing + member management.
//!
//! Thin wrappers over [`swarmnote_core::workspace::sharing`]. The owner grants
//! a paired device Collaborator access (a signed permission op, broadcast to
//! members); the granted device then pulls the workspace via the normal sync
//! flow (its key request succeeds because the owner now recognizes its role).

use std::sync::Arc;

use swarmnote_core::workspace::sharing::{self, MemberInfo};
use swarmnote_core::AppCore;
use tauri::State;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

fn parse_uuid(s: &str) -> AppResult<Uuid> {
    Uuid::parse_str(s).map_err(|e| AppError::InvalidPath(format!("Invalid UUID: {e}")))
}

/// Grant a paired device Collaborator access to the workspace (owner only).
#[tauri::command]
#[specta::specta]
pub async fn share_workspace_to_device(
    workspace_uuid: String,
    target_peer_id: String,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<()> {
    sharing::grant_collaborator(core.inner(), parse_uuid(&workspace_uuid)?, &target_peer_id).await
}

/// List the workspace's current members (role + device presence).
#[tauri::command]
#[specta::specta]
pub async fn list_workspace_members(
    workspace_uuid: String,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<Vec<MemberInfo>> {
    sharing::list_members(core.inner(), parse_uuid(&workspace_uuid)?).await
}

/// Revoke a member's access (owner only).
#[tauri::command]
#[specta::specta]
pub async fn revoke_workspace_member(
    workspace_uuid: String,
    target_peer_id: String,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<()> {
    sharing::revoke_member(core.inner(), parse_uuid(&workspace_uuid)?, &target_peer_id).await
}

/// 邀请已配对设备协作此工作区(owner 发起)。**阻塞等待对方接受/拒绝**,
/// 返回是否被接受;接受后内部已签发授权 op。
#[tauri::command]
#[specta::specta]
pub async fn invite_to_workspace(
    workspace_uuid: String,
    target_peer_id: String,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<bool> {
    core.inner()
        .invite_device(parse_uuid(&workspace_uuid)?, &target_peer_id)
        .await
}

/// 被邀请方应答一条分享邀请(接受/拒绝)。`pending_id` 来自
/// `ShareInvitationReceived` 事件。
#[tauri::command]
#[specta::specta]
pub async fn respond_share_invitation(
    pending_id: u64,
    accept: bool,
    core: State<'_, Arc<AppCore>>,
) -> AppResult<()> {
    core.inner()
        .respond_share_invitation(pending_id, accept)
        .await
}
