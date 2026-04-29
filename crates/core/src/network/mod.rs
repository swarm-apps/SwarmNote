//! P2P 网络层：节点启停、事件循环、DHT 在线宣告。
//!
//! [`NetManager`] 是 P2P 一次运行的"会话包"——启动节点时创建，停止时销毁。
//! 包含 [`DeviceManager`] / [`PairingManager`] / [`OnlineAnnouncer`] 和
//! 取消令牌。上层 [`AppCore`] 以 `Mutex<Option<Arc<NetManager>>>` 持有它，
//! 并通过 `AppCore::start_network` / `stop_network` 控制生命周期。

pub mod config;
pub mod dht_key;
pub mod event_loop;
pub mod online;

use std::sync::Arc;

use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use swarm_p2p_core::libp2p::PeerId;
use swarm_p2p_core::NetClient;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::device::DeviceManager;
use crate::pairing::{PairedDeviceInfo, PairingManager};
use crate::protocol::{AppRequest, AppResponse};

use self::online::OnlineAnnouncer;

/// SwarmNote 对 `swarm-p2p-core` 的请求/响应类型做的 NetClient 特化。
pub type AppNetClient = NetClient<AppRequest, AppResponse>;

/// P2P 节点状态——Rust/前端共用的 single source of truth。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum NodeStatus {
    Stopped,
    Running,
    Error { message: String },
}

/// 网络管理器：持有 P2P 节点所有会话级状态。
///
/// 每次 `AppCore::start_network` 构造一个新实例；`stop_network` 时整体丢弃。
/// 不跨网络生命周期存活（`active_code` / `pending_inbound` 随之清零）。
pub struct NetManager {
    pub(crate) client: AppNetClient,
    pub(crate) peer_id: PeerId,
    pub(crate) device_manager: Arc<DeviceManager>,
    pub(crate) online_announcer: Arc<OnlineAnnouncer>,
    pub(crate) pairing_manager: Arc<PairingManager>,
    cancel_token: CancellationToken,
}

impl NetManager {
    /// 构造一个新的会话。调用方（AppCore）负责在构造后启动事件循环 + online
    /// announce 等后台任务。
    pub fn new(
        client: AppNetClient,
        peer_id: PeerId,
        db: DatabaseConnection,
        device_name: Option<String>,
    ) -> Self {
        let paired_devices: Arc<DashMap<PeerId, PairedDeviceInfo>> = Arc::new(DashMap::new());
        let device_manager = Arc::new(DeviceManager::new(paired_devices.clone()));
        let online_announcer = Arc::new(OnlineAnnouncer::new(client.clone(), peer_id));
        let pairing_manager = Arc::new(PairingManager::new(
            client.clone(),
            peer_id,
            db,
            paired_devices,
            device_name,
        ));
        let cancel_token = CancellationToken::new();

        Self {
            client,
            peer_id,
            device_manager,
            online_announcer,
            pairing_manager,
            cancel_token,
        }
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    // ── Accessors ─────────────────────────────────────────────

    pub fn client(&self) -> &AppNetClient {
        &self.client
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn device_manager(&self) -> &Arc<DeviceManager> {
        &self.device_manager
    }

    pub fn online_announcer(&self) -> &Arc<OnlineAnnouncer> {
        &self.online_announcer
    }

    pub fn pairing_manager(&self) -> &Arc<PairingManager> {
        &self.pairing_manager
    }

    /// 优雅关闭：通知 DHT 下线 → 取消后台任务。
    pub async fn shutdown(&self) {
        // 尝试 announce_offline，但不阻塞关闭流程
        if let Err(e) = self.online_announcer.announce_offline().await {
            warn!("Failed to announce offline: {e}");
        }
        self.cancel_token.cancel();
    }
}
