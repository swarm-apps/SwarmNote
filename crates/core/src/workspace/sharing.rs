//! Workspace sharing operations (owner side): grant/revoke a paired device's
//! membership and list current members. Grants/revokes are recorded as signed
//! [`permissions`] ops and broadcast over the ctrl topic so members converge.
//!
//! v1 grants the **Collaborator** role only (read+write); Reader is reserved
//! for v2. The granted device pulls the workspace via the existing sync flow —
//! its key request succeeds because the owner now recognizes its role
//! (see `coordinator::build_sealed_workspace_key`).

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::permissions::{self, OpKind, Role};
use crate::app::AppCore;
use crate::device::{Device, DeviceFilter};
use crate::error::{AppError, AppResult};

/// A workspace member (materialized role + device presence), for the
/// member-management UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MemberInfo {
    pub peer_id: String,
    pub role: Role,
    pub name: Option<String>,
    pub os: String,
    pub is_online: bool,
    /// `true` for this device's own row (the owner) — UI hides revoke/role edit.
    pub is_self: bool,
}

/// Owner-only: issue a signed permission op, persist it, and broadcast the
/// updated chain to workspace members.
async fn issue_owner_op(
    core: &Arc<AppCore>,
    ws_id: Uuid,
    op_kind: OpKind,
    target_peer_id: &str,
    new_role: Option<Role>,
) -> AppResult<()> {
    let ws = core
        .get_workspace(&ws_id)
        .await
        .ok_or(AppError::NoWorkspaceOpen)?;
    let db = ws.db();
    let identity = core.identity();
    let me = identity.peer_id()?;

    if permissions::role_of(db, ws_id, &me).await? != Some(Role::Owner) {
        return Err(AppError::PermissionDenied(
            "only the workspace owner can manage members".into(),
        ));
    }
    if target_peer_id == me {
        return Err(AppError::PermissionDenied(
            "cannot change your own membership".into(),
        ));
    }

    let key_version = ws.keys().await.current_version().unwrap_or(1) as i32;
    let prev = permissions::chain_tip(&permissions::load_ops(db, ws_id).await?);
    let op = permissions::build_signed_op(
        identity,
        op_kind,
        target_peer_id,
        new_role,
        key_version,
        prev,
    )?;
    permissions::save_op(db, ws_id, &op).await?;

    // Broadcast the full chain so any subscribed member converges.
    if let Some(sync) = ws.sync().await {
        sync.publish_permission_ops(permissions::load_ops(db, ws_id).await?)
            .await;
    }
    Ok(())
}

/// Grant a paired device Collaborator access to the workspace.
pub async fn grant_collaborator(
    core: &Arc<AppCore>,
    ws_id: Uuid,
    target_peer_id: &str,
) -> AppResult<()> {
    issue_owner_op(
        core,
        ws_id,
        OpKind::Grant,
        target_peer_id,
        Some(Role::Collaborator),
    )
    .await
}

/// Revoke a member's access. Lazy: the device keeps any key it already holds
/// (so it can still read content it synced before), but its next key request
/// is denied and it receives no rotated key. True cut-off needs v2 key rotation.
pub async fn revoke_member(
    core: &Arc<AppCore>,
    ws_id: Uuid,
    target_peer_id: &str,
) -> AppResult<()> {
    issue_owner_op(core, ws_id, OpKind::Revoke, target_peer_id, None).await
}

/// List the workspace's current members (materialized roles joined with device
/// presence). Pure read.
pub async fn list_members(core: &Arc<AppCore>, ws_id: Uuid) -> AppResult<Vec<MemberInfo>> {
    let ws = core
        .get_workspace(&ws_id)
        .await
        .ok_or(AppError::NoWorkspaceOpen)?;
    let roles = permissions::load_and_materialize(ws.db(), ws_id).await?;
    let me = core.identity().peer_id()?;
    let my_info = core.identity().device_info()?;

    // Online set + device metadata from the running P2P node (if any).
    let (online, devices): (Vec<String>, Vec<Device>) = match core.net().await {
        Some(net) => (
            net.device_manager
                .connected_paired_peers()
                .iter()
                .map(|p| p.to_string())
                .collect(),
            net.device_manager.get_devices(DeviceFilter::All),
        ),
        None => (Vec::new(), Vec::new()),
    };
    let dev_by_peer: HashMap<&str, &Device> =
        devices.iter().map(|d| (d.peer_id.as_str(), d)).collect();

    let mut members: Vec<MemberInfo> = roles
        .into_iter()
        .map(|(peer_id, role)| {
            let is_self = peer_id == me;
            let dev = dev_by_peer.get(peer_id.as_str());
            MemberInfo {
                name: if is_self {
                    Some(my_info.device_name.clone())
                } else {
                    dev.and_then(|d| d.name.clone())
                },
                os: if is_self {
                    my_info.os.clone()
                } else {
                    dev.map(|d| d.os.clone()).unwrap_or_default()
                },
                is_online: is_self || online.contains(&peer_id),
                is_self,
                peer_id,
                role,
            }
        })
        .collect();

    // Owner first, then stable by peer_id.
    members.sort_by(|a, b| {
        (a.role != Role::Owner)
            .cmp(&(b.role != Role::Owner))
            .then(a.peer_id.cmp(&b.peer_id))
    });
    Ok(members)
}
