use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel};
use serde::{Deserialize, Serialize};
use swarm_p2p_core::libp2p::core::Multiaddr;
use swarm_p2p_core::libp2p::PeerId;
use tracing::info;

use crate::error::{AppError, AppResult};
use crate::network::dht_key;
use crate::network::AppNetClient;
use crate::protocol::{
    AppRequest, AppResponse, OsInfo, PairingMethod, PairingRefuseReason, PairingRequest,
    PairingResponse,
};

use super::code::PairingCodeInfo;

fn parse_peer_id(s: &str) -> AppResult<PeerId> {
    s.parse().map_err(|e| AppError::PairingOther {
        context: "parse PeerId",
        reason: format!("'{s}': {e}"),
    })
}

/// 入站配对请求的过期时间（秒）。大于前端展示的 90s 倒计时，作为后端缓存宽限。
const PENDING_TTL: Duration = Duration::from_secs(120);

/// 配对码发布到 DHT 的记录内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareCodeRecord {
    pub os_info: OsInfo,
    pub listen_addrs: Vec<Multiaddr>,
    pub timestamp: i64,
}

/// 入站配对请求缓存（等待用户确认）。
struct PendingInbound {
    peer_id: PeerId,
    os_info: OsInfo,
    method: PairingMethod,
    created_at: Instant,
}

/// 已配对设备信息，同时用于运行时缓存和 Tauri Event payload。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDeviceInfo {
    pub peer_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub hostname: String,
    pub os: String,
    pub platform: String,
    pub arch: String,
    pub paired_at: chrono::DateTime<chrono::Utc>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_online: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u64>,
}

/// 配对管理器：生成配对码、处理配对请求、管理已配对设备。
pub struct PairingManager {
    client: AppNetClient,
    peer_id: PeerId,
    db: DatabaseConnection,
    /// 运行时已配对设备缓存（与 DeviceManager 共享）
    paired_devices: Arc<DashMap<PeerId, PairedDeviceInfo>>,
    /// 当前用户自定义设备名称（节点生命周期内不变，改名会重启节点）
    device_name: Option<String>,
    /// 当前生效的配对码（同一时间只允许一个）
    active_code: Mutex<Option<PairingCodeInfo>>,
    /// 等待用户确认的入站配对请求
    pending_inbound: DashMap<u64, PendingInbound>,
}

impl PairingManager {
    /// 构造新的 PairingManager。
    pub fn new(
        client: AppNetClient,
        peer_id: PeerId,
        db: DatabaseConnection,
        paired_devices: Arc<DashMap<PeerId, PairedDeviceInfo>>,
        device_name: Option<String>,
    ) -> Self {
        Self {
            client,
            peer_id,
            db,
            paired_devices,
            device_name,
            active_code: Mutex::new(None),
            pending_inbound: DashMap::new(),
        }
    }

    /// 构建包含当前设备自定义名称的 OsInfo。
    /// `device_name` 已在构造时经过 hostname 去重判断（None = 与 hostname 相同）。
    fn local_os_info(&self) -> OsInfo {
        OsInfo {
            name: self.device_name.clone(),
            ..Default::default()
        }
    }

    /// 从数据库加载已配对设备到内存缓存。
    pub async fn load_paired_devices(&self) -> AppResult<()> {
        use entity::devices::paired_devices::Entity;

        let models = Entity::find().all(&self.db).await?;
        for model in models {
            let peer_id = parse_peer_id(&model.peer_id)?;

            let info = PairedDeviceInfo {
                peer_id: model.peer_id,
                name: model.name,
                hostname: model.hostname,
                os: model.os.unwrap_or_default(),
                platform: model.platform.unwrap_or_default(),
                arch: model.arch.unwrap_or_default(),
                paired_at: model.paired_at,
                last_seen: model.last_seen,
                is_online: None,
                rtt_ms: None,
            };

            self.paired_devices.insert(peer_id, info);
        }

        info!(
            "Loaded {} paired device(s) from database",
            self.paired_devices.len()
        );
        Ok(())
    }

    /// 生成配对码并发布到 DHT。
    ///
    /// 同一时间只保留一个有效码，新码会替换旧码。
    pub async fn generate_code(&self, expires_in_secs: u64) -> AppResult<PairingCodeInfo> {
        let code_info = PairingCodeInfo::generate(expires_in_secs);

        // 获取本节点的监听地址
        let listen_addrs = self
            .client
            .get_addrs()
            .await
            .map_err(|e| AppError::SwarmIo {
                context: "get_addrs",
                reason: e.to_string(),
            })?;

        // 构建 DHT 记录
        let record_data = ShareCodeRecord {
            os_info: self.local_os_info(),
            listen_addrs,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let key = dht_key::share_code_key(&code_info.code);
        let value = serde_json::to_vec(&record_data).map_err(|e| AppError::PairingOther {
            context: "serialize ShareCodeRecord",
            reason: e.to_string(),
        })?;

        use swarm_p2p_core::libp2p::kad::Record;
        let record = Record {
            key,
            value,
            publisher: Some(self.peer_id),
            expires: Some(
                std::time::Instant::now() + std::time::Duration::from_secs(expires_in_secs),
            ),
        };

        self.client
            .put_record(record)
            .await
            .map_err(|e| AppError::SwarmIo {
                context: "put_record share_code",
                reason: e.to_string(),
            })?;

        // 存入 active_code
        {
            let mut guard = self.active_code.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(code_info.clone());
        }

        info!("Generated pairing code (expires in {expires_in_secs}s)");
        Ok(code_info)
    }

    /// 通过配对码从 DHT 查找设备信息。
    ///
    /// 返回 `(peer_id_string, ShareCodeRecord)`。
    pub async fn get_device_by_code(&self, code: &str) -> AppResult<(String, ShareCodeRecord)> {
        let key = dht_key::share_code_key(code);

        let result = self
            .client
            .get_record(key)
            .await
            .map_err(|e| AppError::SwarmIo {
                context: "get_record share_code",
                reason: e.to_string(),
            })?;

        let record = result.record;

        // 检查过期
        if record
            .expires
            .is_some_and(|e| e < std::time::Instant::now())
        {
            return Err(AppError::PairingCodeExpired);
        }

        // 提取发布者 PeerId
        let publisher = record.publisher.ok_or(AppError::PairingCodeInvalid)?;

        // 解析记录内容
        let share_record: ShareCodeRecord =
            serde_json::from_slice(&record.value).map_err(|e| AppError::PairingOther {
                context: "deserialize ShareCodeRecord",
                reason: e.to_string(),
            })?;

        // 注册对方地址到 Swarm，以便后续拨号
        if !share_record.listen_addrs.is_empty() {
            self.client
                .add_peer_addrs(publisher, share_record.listen_addrs.clone())
                .await
                .map_err(|e| AppError::SwarmIo {
                    context: "add_peer_addrs",
                    reason: e.to_string(),
                })?;
        }

        Ok((publisher.to_string(), share_record))
    }

    /// 向目标设备发送配对请求。
    ///
    /// 如果对方接受，自动持久化为已配对设备。
    /// `remote_os_info` 为对方设备信息（可从 `get_device_by_code` 获取），
    /// 若未提供则 fallback 到默认空信息（后续 Identify 协议会补齐）。
    pub async fn request_pairing(
        &self,
        peer_id_str: &str,
        method: PairingMethod,
        remote_os_info: Option<OsInfo>,
    ) -> AppResult<PairingResponse> {
        let peer_id = parse_peer_id(peer_id_str)?;

        let request = PairingRequest {
            os_info: self.local_os_info(),
            timestamp: chrono::Utc::now().timestamp(),
            method,
        };

        let response = self
            .client
            .send_request(peer_id, AppRequest::Pairing(request))
            .await
            .map_err(|e| AppError::SwarmIo {
                context: "send_request pairing",
                reason: e.to_string(),
            })?;

        match response {
            AppResponse::Pairing(pairing_resp) => {
                if matches!(pairing_resp, PairingResponse::Success) {
                    let os_info = remote_os_info.unwrap_or_default();
                    self.persist_paired_device(peer_id, &os_info).await?;
                }

                Ok(pairing_resp)
            }
            _ => Err(AppError::PairingOther {
                context: "send_request response",
                reason: "unexpected response type (expected Pairing)".to_string(),
            }),
        }
    }

    /// 缓存一个入站配对请求，等待用户在前端确认/拒绝。
    ///
    /// 同时清理超过 `PENDING_TTL` 的过期条目。
    pub fn cache_inbound_request(
        &self,
        peer_id: PeerId,
        pending_id: u64,
        request: &PairingRequest,
    ) {
        // 清理过期的 pending 条目
        self.pending_inbound
            .retain(|_, v| v.created_at.elapsed() < PENDING_TTL);

        self.pending_inbound.insert(
            pending_id,
            PendingInbound {
                peer_id,
                os_info: request.os_info.clone(),
                method: request.method.clone(),
                created_at: Instant::now(),
            },
        );
    }

    /// 处理用户对入站配对请求的确认或拒绝。
    ///
    /// - `accept = true`: 验证配对码（如适用）→ 回复 Success → 持久化
    /// - `accept = false`: 回复 Refused(UserRejected)
    pub async fn handle_pairing_request(
        &self,
        pending_id: u64,
        accept: bool,
    ) -> AppResult<Option<PairedDeviceInfo>> {
        let (_, pending) = self
            .pending_inbound
            .remove(&pending_id)
            .ok_or(AppError::PairingPendingNotFound(pending_id))?;

        // H-2: 检查 pending 是否已超时
        if pending.created_at.elapsed() >= PENDING_TTL {
            self.client
                .send_response(
                    pending_id,
                    AppResponse::Pairing(PairingResponse::Refused {
                        reason: PairingRefuseReason::CodeExpired,
                    }),
                )
                .await
                .map_err(|e| AppError::SwarmIo {
                    context: "send_response pairing expired",
                    reason: e.to_string(),
                })?;
            return Err(AppError::PairingCodeExpired);
        }

        if !accept {
            self.client
                .send_response(
                    pending_id,
                    AppResponse::Pairing(PairingResponse::Refused {
                        reason: PairingRefuseReason::UserRejected,
                    }),
                )
                .await
                .map_err(|e| AppError::SwarmIo {
                    context: "send_response pairing rejected",
                    reason: e.to_string(),
                })?;
            return Ok(None);
        }

        // C-1: 如果是 Code 方式，验证 active_code 存在、未过期、且 code 匹配
        if let PairingMethod::Code { ref code } = pending.method {
            let refuse_reason = {
                let guard = self.active_code.lock().unwrap_or_else(|e| e.into_inner());
                match guard.as_ref() {
                    None => Some(PairingRefuseReason::CodeInvalid),
                    Some(active) if active.is_expired() => Some(PairingRefuseReason::CodeExpired),
                    Some(active) if active.code != *code => Some(PairingRefuseReason::CodeInvalid),
                    _ => None,
                }
            };

            if let Some(reason) = refuse_reason {
                self.client
                    .send_response(
                        pending_id,
                        AppResponse::Pairing(PairingResponse::Refused {
                            reason: reason.clone(),
                        }),
                    )
                    .await
                    .map_err(|e| AppError::SwarmIo {
                        context: "send_response pairing code refused",
                        reason: e.to_string(),
                    })?;
                return Err(match reason {
                    PairingRefuseReason::CodeExpired => AppError::PairingCodeExpired,
                    PairingRefuseReason::CodeInvalid => AppError::PairingCodeInvalid,
                    _ => AppError::PairingCodeInvalid,
                });
            }

            // 消费配对码（一次性使用），仅 Code 模式
            {
                let mut guard = self.active_code.lock().unwrap_or_else(|e| e.into_inner());
                *guard = None;
            }
        }

        // 回复 Success
        self.client
            .send_response(pending_id, AppResponse::Pairing(PairingResponse::Success))
            .await
            .map_err(|e| AppError::SwarmIo {
                context: "send_response pairing success",
                reason: e.to_string(),
            })?;

        // 持久化已配对设备
        let info = self
            .persist_paired_device(pending.peer_id, &pending.os_info)
            .await?;

        Ok(Some(info))
    }

    /// 取消配对：从内存缓存和数据库中移除设备。
    pub async fn unpair(&self, peer_id_str: &str) -> AppResult<()> {
        use entity::devices::paired_devices::Entity;

        let peer_id = parse_peer_id(peer_id_str)?;

        self.paired_devices.remove(&peer_id);
        Entity::delete_by_id(peer_id_str).exec(&self.db).await?;

        info!("Unpaired device: {peer_id_str}");
        Ok(())
    }

    /// 返回所有已配对设备的快照。
    pub fn get_paired_devices(&self) -> Vec<PairedDeviceInfo> {
        self.paired_devices
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 返回所有已配对设备的 PeerId（供 `check_paired_online` 使用）。
    pub fn get_paired_peer_ids(&self) -> Vec<PeerId> {
        self.paired_devices
            .iter()
            .map(|entry| *entry.key())
            .collect()
    }

    /// 持久化已配对设备到 DB 和内存缓存。
    async fn persist_paired_device(
        &self,
        peer_id: PeerId,
        os_info: &OsInfo,
    ) -> AppResult<PairedDeviceInfo> {
        use entity::devices::paired_devices;

        let peer_id_str = peer_id.to_string();
        let now = chrono::Utc::now();

        let info = PairedDeviceInfo {
            peer_id: peer_id_str.clone(),
            name: os_info.name.clone(),
            hostname: os_info.hostname.clone(),
            os: os_info.os.clone(),
            platform: os_info.platform.clone(),
            arch: os_info.arch.clone(),
            paired_at: now,
            last_seen: Some(now),
            is_online: Some(true),
            rtt_ms: None,
        };

        let model = paired_devices::Model {
            peer_id: peer_id_str,
            name: os_info.name.clone(),
            hostname: os_info.hostname.clone(),
            os: Some(os_info.os.clone()),
            platform: Some(os_info.platform.clone()),
            arch: Some(os_info.arch.clone()),
            paired_at: now,
            last_seen: Some(now),
        };

        // M-4: 先尝试删除已有记录（不存在时忽略错误）
        let _ = paired_devices::Entity::delete_by_id(&info.peer_id)
            .exec(&self.db)
            .await;

        model.into_active_model().insert(&self.db).await?;
        self.paired_devices.insert(peer_id, info.clone());

        info!("Paired device persisted: {}", info.peer_id);
        Ok(info)
    }
}
