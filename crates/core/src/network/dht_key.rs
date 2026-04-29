use sha2::{Digest, Sha256};
use swarm_p2p_core::libp2p::kad::RecordKey;

const NS_ONLINE: &[u8] = b"/swarmnote/online/";
const NS_SHARE_CODE: &[u8] = b"/swarmnote/share-code/";

/// 生成带命名空间的 DHT key: SHA256(namespace || id)
fn dht_key(namespace: &[u8], id: &[u8]) -> RecordKey {
    Sha256::digest([namespace, id].concat()).to_vec().into()
}

/// 在线宣告的 DHT key
pub fn online_key(peer_id_bytes: &[u8]) -> RecordKey {
    dht_key(NS_ONLINE, peer_id_bytes)
}

/// 配对码的 DHT key
pub fn share_code_key(code: &str) -> RecordKey {
    dht_key(NS_SHARE_CODE, code.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_key_deterministic() {
        let peer_bytes = b"test-peer-id";
        let key1 = online_key(peer_bytes);
        let key2 = online_key(peer_bytes);
        assert_eq!(key1, key2);
    }

    #[test]
    fn different_peers_different_keys() {
        let key1 = online_key(b"peer-1");
        let key2 = online_key(b"peer-2");
        assert_ne!(key1, key2);
    }

    #[test]
    fn different_namespaces_different_keys() {
        let data = b"same-data";
        let online = online_key(data);
        let code = share_code_key("same-data");
        assert_ne!(online, code);
    }
}
