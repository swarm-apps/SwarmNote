use std::sync::Arc;

use dashmap::DashMap;
use swarm_p2p_core::libp2p::{Multiaddr, PeerId};
use swarm_p2p_core::NodeEvent;

use super::utils::infer_connection_type;
use super::{ConnectionType, Device, DeviceStatus};
use crate::pairing::PairedDeviceInfo;
use crate::protocol::{AppRequest, OsInfo};

/// 运行时 Peer 信息（DashMap 中的值，不直接序列化给前端）
#[derive(Debug, Clone)]
pub(super) struct PeerInfo {
    pub peer_id: PeerId,
    pub addrs: Vec<Multiaddr>,
    pub agent_version: Option<String>,
    pub rtt_ms: Option<u64>,
    pub is_connected: bool,
    /// DCUtR 打洞是否成功（比地址推断更准确）
    pub hole_punched: bool,
    #[expect(dead_code)]
    pub discovered_at: i64,
    pub connected_at: Option<i64>,
}

impl PeerInfo {
    fn new_discovered(peer_id: PeerId, addrs: Vec<Multiaddr>) -> Self {
        Self {
            peer_id,
            addrs,
            agent_version: None,
            rtt_ms: None,
            is_connected: false,
            hole_punched: false,
            discovered_at: chrono::Utc::now().timestamp_millis(),
            connected_at: None,
        }
    }
}

/// 设备过滤器
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DeviceFilter {
    #[default]
    All,
    Connected,
    Paired,
}

/// 设备管理器
///
/// 维护运行时发现的 peer，提供统一的设备查询接口。
/// 已配对设备通过共享 `Arc<DashMap>` 从 [`PairingManager`] 读取。
pub struct DeviceManager {
    peers: DashMap<PeerId, PeerInfo>,
    /// 与 PairingManager 共享的已配对设备（只读）
    paired_devices: Arc<DashMap<PeerId, PairedDeviceInfo>>,
}

impl DeviceManager {
    pub fn new(paired_devices: Arc<DashMap<PeerId, PairedDeviceInfo>>) -> Self {
        Self {
            peers: DashMap::new(),
            paired_devices,
        }
    }

    /// 处理 NodeEvent，更新 peer 状态
    pub fn handle_event(&self, event: &NodeEvent<AppRequest>) {
        match event {
            NodeEvent::PeersDiscovered { peers } => {
                for (peer_id, addr) in peers {
                    match self.peers.get_mut(peer_id) {
                        Some(mut entry) => {
                            if !entry.addrs.contains(addr) {
                                entry.addrs.push(addr.clone());
                            }
                        }
                        None => {
                            self.peers.insert(
                                *peer_id,
                                PeerInfo::new_discovered(*peer_id, vec![addr.clone()]),
                            );
                        }
                    }
                }
            }

            NodeEvent::PeerConnected { peer_id } => {
                let now = chrono::Utc::now().timestamp_millis();
                match self.peers.get_mut(peer_id) {
                    Some(mut entry) => {
                        entry.is_connected = true;
                        entry.connected_at = Some(now);
                    }
                    None => {
                        let mut info = PeerInfo::new_discovered(*peer_id, vec![]);
                        info.is_connected = true;
                        info.connected_at = Some(now);
                        self.peers.insert(*peer_id, info);
                    }
                }
            }

            NodeEvent::PeerDisconnected { peer_id } => {
                if let Some(mut entry) = self.peers.get_mut(peer_id) {
                    entry.is_connected = false;
                    entry.rtt_ms = None;
                    entry.hole_punched = false;
                }
            }

            NodeEvent::IdentifyReceived {
                peer_id,
                agent_version,
                ..
            } => {
                if let Some(mut entry) = self.peers.get_mut(peer_id) {
                    entry.agent_version = Some(agent_version.clone());
                }
                // Update paired device cache with latest name/hostname from agent_version
                if let Some(mut paired) = self.paired_devices.get_mut(peer_id) {
                    if let Some(os_info) = OsInfo::from_agent_version(agent_version) {
                        paired.name = os_info.name;
                        paired.hostname = os_info.hostname;
                    }
                }
            }

            NodeEvent::PingSuccess { peer_id, rtt_ms } => {
                if let Some(mut entry) = self.peers.get_mut(peer_id) {
                    entry.rtt_ms = Some(*rtt_ms);
                }
            }

            NodeEvent::HolePunchSucceeded { peer_id } => {
                if let Some(mut entry) = self.peers.get_mut(peer_id) {
                    entry.hole_punched = true;
                }
            }

            _ => {}
        }
    }

    /// Check whether a peer is in the paired devices list.
    pub fn is_paired(&self, peer_id: &PeerId) -> bool {
        self.paired_devices.contains_key(peer_id)
    }

    /// Get PeerIds of all connected and paired devices (for SV compensation).
    pub fn connected_paired_peers(&self) -> Vec<PeerId> {
        self.paired_devices
            .iter()
            .filter_map(|entry| {
                let peer_id = *entry.key();
                let is_connected = self
                    .peers
                    .get(&peer_id)
                    .map(|p| p.is_connected)
                    .unwrap_or(false);
                if is_connected {
                    Some(peer_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// 统一查询设备列表
    pub fn get_devices(&self, filter: DeviceFilter) -> Vec<Device> {
        match filter {
            DeviceFilter::All | DeviceFilter::Connected => {
                let connected_only = matches!(filter, DeviceFilter::Connected);
                self.peers
                    .iter()
                    .filter(|entry| {
                        let peer = entry.value();
                        let is_app_peer = peer
                            .agent_version
                            .as_deref()
                            .is_some_and(OsInfo::is_swarmnote_agent);
                        is_app_peer && (!connected_only || peer.is_connected)
                    })
                    .map(|entry| self.peer_to_device(entry.value()))
                    .collect()
            }
            DeviceFilter::Paired => self
                .paired_devices
                .iter()
                .filter_map(|entry| {
                    let info = entry.value();
                    let peer_id = info.peer_id.parse::<PeerId>().ok()?;
                    let peer_info = self.peers.get(&peer_id);

                    // Online: use real-time identity from PeerInfo; offline: fall back to DB snapshot
                    let (status, connection, latency) = match peer_info.as_deref() {
                        Some(p) if p.is_connected => {
                            connection_info(&p.addrs, p.rtt_ms, p.hole_punched)
                        }
                        _ => (DeviceStatus::Offline, None, None),
                    };

                    let live_os = peer_info
                        .as_deref()
                        .filter(|p| p.is_connected)
                        .and_then(|p| p.agent_version.as_deref())
                        .and_then(OsInfo::from_agent_version);

                    let (name, hostname, os, platform, arch) = match live_os {
                        Some(oi) => (oi.name, oi.hostname, oi.os, oi.platform, oi.arch),
                        None => (
                            info.name.clone(),
                            info.hostname.clone(),
                            info.os.clone(),
                            info.platform.clone(),
                            info.arch.clone(),
                        ),
                    };

                    Some(Device {
                        peer_id: info.peer_id.clone(),
                        name,
                        hostname,
                        os,
                        platform,
                        arch,
                        status,
                        connection,
                        latency,
                        is_paired: true,
                        paired_at: Some(info.paired_at),
                        last_seen: info.last_seen,
                    })
                })
                .collect(),
        }
    }

    /// 将 PeerInfo 转换为 Device
    fn peer_to_device(&self, peer: &PeerInfo) -> Device {
        let os_info = peer
            .agent_version
            .as_deref()
            .and_then(OsInfo::from_agent_version)
            .unwrap_or_else(|| OsInfo::unknown_from_peer_id(&peer.peer_id));

        let (status, connection, latency) = if peer.is_connected {
            connection_info(&peer.addrs, peer.rtt_ms, peer.hole_punched)
        } else {
            (DeviceStatus::Offline, None, None)
        };

        let paired = self.paired_devices.get(&peer.peer_id);

        Device {
            peer_id: peer.peer_id.to_string(),
            name: os_info.name,
            hostname: os_info.hostname,
            os: os_info.os,
            platform: os_info.platform,
            arch: os_info.arch,
            status,
            connection,
            latency,
            is_paired: paired.is_some(),
            paired_at: paired.as_ref().map(|entry| entry.paired_at),
            last_seen: paired.as_ref().and_then(|entry| entry.last_seen),
        }
    }

    /// 已连接的 SwarmNote 客户端数量
    pub fn connected_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|e| {
                let p = e.value();
                p.is_connected
                    && p.agent_version
                        .as_deref()
                        .is_some_and(OsInfo::is_swarmnote_agent)
            })
            .count()
    }
}

/// 根据连接状态提取 (DeviceStatus, ConnectionType, latency)
fn connection_info(
    addrs: &[Multiaddr],
    rtt_ms: Option<u64>,
    hole_punched: bool,
) -> (DeviceStatus, Option<ConnectionType>, Option<u64>) {
    let connection = if hole_punched {
        Some(ConnectionType::Dcutr)
    } else {
        infer_connection_type(addrs)
    };
    (DeviceStatus::Online, connection, rtt_ms)
}
