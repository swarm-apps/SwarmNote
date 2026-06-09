//! P2P 事件循环：接收 NodeEvent 并分发到 DeviceManager、EventBus、Sync 模块。

use std::sync::Arc;

use sea_orm::{EntityTrait, PaginatorTrait};
use swarm_p2p_core::event::NodeEvent;
use swarm_p2p_core::libp2p::PeerId;
use swarm_p2p_core::EventReceiver;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::app::AppCore;
use crate::device::{DeviceFilter, DeviceManager};
use crate::events::AppEvent;
use crate::network::AppNetClient;
use crate::pairing::PairingManager;
use crate::protocol::{
    AppRequest, AppResponse, WorkspaceMeta, WorkspaceRequest, WorkspaceResponse,
};
use crate::workspace::sync::{parse_ws_awareness_topic, parse_ws_topic, AppSyncCoordinator};

/// 启动事件循环，持续读取 NodeEvent 并分发到 DeviceManager + EventBus。
///
/// 在 [`AppCore::start_network`] 构造 [`crate::network::NetManager`] 成功后调用，
/// 由 `cancel_token` 统一关闭——与 `NetManager.cancel_token` 同源。
pub fn spawn_event_loop(
    mut receiver: EventReceiver<AppRequest>,
    core: Arc<AppCore>,
    client: AppNetClient,
    device_manager: Arc<DeviceManager>,
    pairing_manager: Arc<PairingManager>,
    coordinator: Arc<AppSyncCoordinator>,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    info!("Event loop cancelled");
                    break;
                }
                event = receiver.recv() => {
                    match event {
                        Some(event) => handle_event(
                            event,
                            &core,
                            &client,
                            &device_manager,
                            &pairing_manager,
                            &coordinator,
                        ).await,
                        None => {
                            info!("Event receiver closed, exiting event loop");
                            break;
                        }
                    }
                }
            }
        }
    });
}

async fn handle_event(
    event: NodeEvent<AppRequest>,
    core: &Arc<AppCore>,
    client: &AppNetClient,
    device_manager: &DeviceManager,
    pairing_manager: &PairingManager,
    coordinator: &Arc<AppSyncCoordinator>,
) {
    // 统一更新设备状态
    device_manager.handle_event(&event);

    let emit_devices = || {
        let devices = device_manager.get_devices(DeviceFilter::All);
        core.event_bus.emit(AppEvent::DevicesChanged { devices });
    };

    match event {
        // ── 设备发现与连接 ──
        NodeEvent::Listening { addr } => {
            info!("Listening on {addr}");
        }
        NodeEvent::PeersDiscovered { ref peers } => {
            info!("Discovered {} peer(s)", peers.len());
            emit_devices();
        }
        NodeEvent::PeerConnected { ref peer_id } => {
            info!("Peer connected: {peer_id}");
            emit_devices();
        }
        NodeEvent::PeerDisconnected { ref peer_id } => {
            info!("Peer disconnected: {peer_id}");
            emit_devices();
        }
        NodeEvent::IdentifyReceived {
            ref peer_id,
            ref agent_version,
            ..
        } => {
            info!("Identified peer: {peer_id} ({agent_version})");
            emit_devices();

            // If the identified peer is a paired device, trigger full sync
            if device_manager.is_paired(peer_id) {
                coordinator.on_paired_peer_connected(*peer_id).await;
            }
        }
        NodeEvent::PingSuccess { .. } => {
            emit_devices();
        }
        NodeEvent::HolePunchSucceeded { ref peer_id } => {
            info!("Hole punch succeeded with {peer_id}");
            emit_devices();
        }

        // ── 网络状态 ──
        NodeEvent::NatStatusChanged {
            status,
            public_addr,
        } => {
            info!("NAT status changed: {status:?}, public_addr: {public_addr:?}");
            core.event_bus.emit(AppEvent::NetworkStatusChanged {
                nat_status: format!("{status:?}"),
                public_addr: public_addr.map(|a| a.to_string()),
            });
        }
        NodeEvent::HolePunchFailed { peer_id, error } => {
            warn!("Hole punch failed with {peer_id}: {error}");
        }
        NodeEvent::RelayReservationAccepted {
            relay_peer_id,
            renewal,
        } => {
            info!(
                "Relay reservation {} by {relay_peer_id}",
                if renewal { "renewed" } else { "accepted" }
            );
        }

        // ── 入站请求 ──
        NodeEvent::InboundRequest {
            peer_id,
            pending_id,
            request,
        } => {
            handle_inbound_request(
                core,
                client,
                pairing_manager,
                coordinator,
                peer_id,
                pending_id,
                request,
            )
            .await;
        }

        // ── GossipSub ──
        NodeEvent::GossipMessage {
            source,
            topic,
            data,
        } => {
            if topic == crate::workspace::sync::CTRL_TOPIC {
                if let (Some(peer), Some(msg)) =
                    (source, crate::workspace::sync::decode_ctrl_message(&data))
                {
                    if device_manager.is_paired(&peer) {
                        coordinator.handle_ctrl_message(peer, msg).await;
                    }
                }
            } else if let Some(ws_uuid) = parse_ws_topic(&topic) {
                // Encrypted workspace doc-update broadcast — decrypt + route.
                coordinator
                    .handle_ws_gossip_update(source, ws_uuid, data)
                    .await;
            } else if let Some(ws_uuid) = parse_ws_awareness_topic(&topic) {
                // Encrypted workspace awareness broadcast — decrypt + fan-out.
                coordinator
                    .handle_ws_awareness_gossip(source, ws_uuid, data)
                    .await;
            } else {
                info!("GossipSub message on unknown topic: {topic}");
            }
        }
        NodeEvent::GossipSubscribed { peer_id, topic } => {
            info!("Peer {peer_id} subscribed to topic {topic}");
        }
        NodeEvent::GossipUnsubscribed { peer_id, topic } => {
            info!("Peer {peer_id} unsubscribed from topic {topic}");
        }
    }
}

// ── 入站请求处理 ──

async fn handle_inbound_request(
    core: &Arc<AppCore>,
    client: &AppNetClient,
    pairing_manager: &PairingManager,
    coordinator: &AppSyncCoordinator,
    peer_id: PeerId,
    pending_id: u64,
    request: AppRequest,
) {
    match &request {
        AppRequest::Pairing(pairing_req) => {
            info!("Received pairing request from {peer_id} (pending_id={pending_id})");
            pairing_manager.cache_inbound_request(peer_id, pending_id, pairing_req);

            let expires_at = chrono::Utc::now() + chrono::Duration::seconds(90);
            core.event_bus.emit(AppEvent::PairingRequestReceived {
                pending_id,
                peer_id: peer_id.to_string(),
                os_info: pairing_req.os_info.clone(),
                method: pairing_req.method.clone(),
                expires_at,
            });
        }

        AppRequest::Workspace(WorkspaceRequest::ListWorkspaces) => {
            info!("Received ListWorkspaces request from {peer_id}");
            let response = build_workspace_list(core, peer_id).await;
            if let Err(e) = client
                .send_response(pending_id, AppResponse::Workspace(response))
                .await
            {
                warn!("Failed to send WorkspaceList response to {peer_id}: {e}");
            }
        }

        AppRequest::Workspace(WorkspaceRequest::ShareInvitation {
            workspace_uuid,
            name,
        }) => {
            // 入站分享邀请:不立即响应,弹给用户决定。请求方的 send_request 在
            // 此期间阻塞;用户接受/拒绝后经 respond_share_invitation 回填
            // (send_response(pending_id, ...))。
            info!(
                "Received share invitation for {workspace_uuid} from {peer_id} (pending_id={pending_id})"
            );
            let expires_at = chrono::Utc::now() + chrono::Duration::seconds(90);
            core.event_bus.emit(AppEvent::ShareInvitationReceived {
                pending_id,
                peer_id: peer_id.to_string(),
                workspace_uuid: *workspace_uuid,
                workspace_name: name.clone(),
                expires_at,
            });
        }

        AppRequest::Sync(sync_req) => {
            coordinator
                .handle_inbound_request(peer_id, pending_id, sync_req.clone())
                .await;
        }
    }
}

/// 构建工作区元数据列表，**只包含请求方被授权访问的工作区**（与 key 分发 /
/// 同步响应的权限 gating 一致，避免请求方"看得到却拉不动")。
///
/// 候选集 = 已打开的工作区 ∪ 最近列表里**未打开**的工作区——后者会被
/// [`AppCore::ensure_open_for_sync`] 以 headless / sync-only 方式懒打开,使本设备
/// 即使没在窗口里打开某工作区,授权的对端也能发现并拉取它(仍按 role 过滤)。
async fn build_workspace_list(core: &Arc<AppCore>, requester: PeerId) -> WorkspaceResponse {
    use entity::workspace::documents;

    let requester_str = requester.to_string();

    // 收集候选 UUID:先已打开的,再补上最近列表里尚未打开的(去重)。
    let mut candidates: Vec<uuid::Uuid> = core
        .list_workspaces()
        .await
        .iter()
        .map(|w| w.info.id)
        .collect();
    for rw in core.recent_workspaces().await {
        if let Some(uuid) = rw
            .uuid
            .as_deref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
        {
            if !candidates.contains(&uuid) {
                candidates.push(uuid);
            }
        }
    }

    let mut metas = Vec::new();
    for uuid in candidates {
        // 懒打开(已打开则直接拿到;未打开则 sync-only 打开)。
        let Some(ws) = core.ensure_open_for_sync(uuid).await else {
            continue;
        };
        // 只广播请求方有角色的工作区。
        let authorized = matches!(
            crate::workspace::permissions::role_of(ws.db(), uuid, &requester_str).await,
            Ok(Some(_))
        );
        if !authorized {
            continue;
        }

        let doc_count = documents::Entity::find().count(ws.db()).await.unwrap_or(0) as u32;

        metas.push(WorkspaceMeta {
            uuid,
            name: ws.info.name.clone(),
            doc_count,
            updated_at: ws.info.updated_at.timestamp_millis(),
        });
    }

    WorkspaceResponse::WorkspaceList { workspaces: metas }
}
