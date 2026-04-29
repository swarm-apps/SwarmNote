use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use swarm_p2p_core::libp2p::core::Multiaddr;
use swarm_p2p_core::libp2p::kad::Record;
use swarm_p2p_core::libp2p::PeerId;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::protocol::OsInfo;

use super::{dht_key, AppNetClient};

/// DHT 在线宣告记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineRecord {
    #[serde(flatten)]
    pub os_info: OsInfo,
    #[serde(default)]
    pub listen_addrs: Vec<Multiaddr>,
    pub timestamp: i64,
}

/// DHT 在线宣告 TTL（秒）
const ONLINE_TTL_SECS: u64 = 300;
/// 续期间隔（秒），需小于 TTL
const RENEWAL_INTERVAL_SECS: u64 = 240;

/// 管理 DHT 在线宣告：发布、续期、移除、已配对设备重连
pub struct OnlineAnnouncer {
    client: AppNetClient,
    peer_id: PeerId,
}

impl OnlineAnnouncer {
    pub fn new(client: AppNetClient, peer_id: PeerId) -> Self {
        Self { client, peer_id }
    }

    /// 发布在线宣告到 DHT
    pub async fn announce_online(&self) -> crate::error::AppResult<()> {
        let addrs = self
            .client
            .get_addrs()
            .await
            .map_err(|e| crate::error::AppError::SwarmIo {
                context: "get_addrs",
                reason: e.to_string(),
            })?;

        let record_data = OnlineRecord {
            os_info: OsInfo::default(),
            listen_addrs: addrs,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let key = dht_key::online_key(&self.peer_id.to_bytes());
        let value =
            serde_json::to_vec(&record_data).map_err(|e| crate::error::AppError::SwarmIo {
                context: "serialize OnlineRecord",
                reason: e.to_string(),
            })?;

        let record = Record {
            key,
            value,
            publisher: Some(self.peer_id),
            expires: Some(
                std::time::Instant::now() + std::time::Duration::from_secs(ONLINE_TTL_SECS),
            ),
        };

        self.client
            .put_record(record)
            .await
            .map_err(|e| crate::error::AppError::SwarmIo {
                context: "put_record online",
                reason: e.to_string(),
            })?;

        info!("Published online announcement to DHT");
        Ok(())
    }

    /// 从 DHT 移除在线宣告
    pub async fn announce_offline(&self) -> crate::error::AppResult<()> {
        let key = dht_key::online_key(&self.peer_id.to_bytes());
        self.client
            .remove_record(key)
            .await
            .map_err(|e| crate::error::AppError::SwarmIo {
                context: "remove_record online",
                reason: e.to_string(),
            })?;
        info!("Removed online announcement from DHT");
        Ok(())
    }

    /// 查询已配对设备的在线状态并主动拨号重连
    pub async fn check_paired_online(&self, paired_peer_ids: Vec<PeerId>) {
        if paired_peer_ids.is_empty() {
            return;
        }

        info!(
            "Checking online status for {} paired devices",
            paired_peer_ids.len()
        );

        for peer_id in paired_peer_ids {
            let key = dht_key::online_key(&peer_id.to_bytes());
            match self.client.get_record(key).await {
                Ok(result) => {
                    let record = result.record;
                    // 跳过已过期记录
                    if record
                        .expires
                        .is_some_and(|e| e < std::time::Instant::now())
                    {
                        continue;
                    }

                    if let Ok(online_record) = serde_json::from_slice::<OnlineRecord>(&record.value)
                    {
                        if online_record.listen_addrs.is_empty() {
                            continue;
                        }
                        if let Err(e) = self
                            .client
                            .add_peer_addrs(peer_id, online_record.listen_addrs)
                            .await
                        {
                            warn!("Failed to register addrs for {peer_id}: {e}");
                            continue;
                        }
                        if let Err(e) = self.client.dial(peer_id).await {
                            warn!("Failed to dial paired device {peer_id}: {e}");
                        } else {
                            info!("Initiated reconnection to paired device {peer_id}");
                        }
                    }
                }
                Err(_) => {
                    // 设备离线或 DHT 查询失败，正常现象
                }
            }
        }
    }

    /// 启动周期续期后台任务
    pub fn spawn_renewal_task(self: Arc<Self>, cancel_token: CancellationToken) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(RENEWAL_INTERVAL_SECS));
            // 跳过首次 tick（启动时已经 announce 了）
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Online announcement renewal task cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        if let Err(e) = self.announce_online().await {
                            warn!("Failed to renew online announcement: {e}");
                        }
                    }
                }
            }
        });
    }
}
