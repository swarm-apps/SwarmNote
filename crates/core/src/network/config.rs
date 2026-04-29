use std::time::Duration;

use swarm_p2p_core::libp2p::core::Multiaddr;
use swarm_p2p_core::libp2p::PeerId;
use swarm_p2p_core::NodeConfig;

/// SwarmDrop 的公共 bootstrap 节点（DHT key 有命名空间隔离，不会冲突）
const BOOTSTRAP_NODES: &[&str] = &[
    "/ip4/47.115.172.218/tcp/4001/p2p/12D3KooWCq8xgrSap7VZZHpW7EYXw8zFmNEgru9D7cGHGW3bMASX",
    "/ip4/47.115.172.218/udp/4001/quic-v1/p2p/12D3KooWCq8xgrSap7VZZHpW7EYXw8zFmNEgru9D7cGHGW3bMASX",
];

/// 创建 P2P 节点配置
pub fn create_node_config(agent_version: String) -> NodeConfig {
    let bootstrap_peers = parse_bootstrap_nodes(BOOTSTRAP_NODES);

    NodeConfig::new("/swarmnote/1.0.0", agent_version)
        .with_mdns(true)
        .with_relay_client(true)
        .with_dcutr(true)
        .with_autonat(true)
        .with_gossipsub(true)
        .with_req_resp_protocol("/swarmnote/req/1.0.0")
        .with_req_resp_timeout(Duration::from_secs(120))
        .with_bootstrap_peers(bootstrap_peers)
}

fn parse_bootstrap_nodes(nodes: &[&str]) -> Vec<(PeerId, Multiaddr)> {
    nodes
        .iter()
        .filter_map(|s| {
            let addr: Multiaddr = s.parse().ok()?;
            // 从 multiaddr 提取 PeerId（最后一个 /p2p/<peer_id> 段）
            let peer_id = addr.iter().find_map(|proto| {
                if let swarm_p2p_core::libp2p::core::multiaddr::Protocol::P2p(peer_id) = proto {
                    Some(peer_id)
                } else {
                    None
                }
            })?;
            Some((peer_id, addr))
        })
        .collect()
}
