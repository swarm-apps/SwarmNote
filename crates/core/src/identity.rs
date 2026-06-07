//! Device identity — Ed25519 keypair + [`DeviceInfo`] (peer_id, device_name,
//! hostname, os/platform/arch, created_at).
//!
//! Keypair persistence is delegated to [`crate::keychain::KeychainProvider`]
//! (desktop: `keyring` crate; mobile: Android Keystore / iOS Keychain).

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use swarm_p2p_core::libp2p::identity::Keypair;
use tracing::info;

use crate::config::GlobalConfig;
use crate::error::{AppError, AppResult};
use crate::keychain::KeychainProvider;

/// Runtime device identity + OS metadata. Returned to the frontend by
/// `get_device_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DeviceInfo {
    pub peer_id: String,
    pub device_name: String,
    pub hostname: String,
    pub os: String,
    pub platform: String,
    pub arch: String,
    pub created_at: String,
}

/// Owns the long-lived device keypair + a mutable `DeviceInfo` snapshot.
///
/// Constructed once during [`crate::AppCore::new`], shared via `Arc`.
pub struct IdentityManager {
    /// Ed25519 keypair used for libp2p authentication.
    /// Read via [`IdentityManager::keypair_protobuf`] when bootstrapping
    /// the P2P swarm.
    keypair: Keypair,
    /// Device metadata snapshot. Only `device_name` mutates at runtime.
    device_info: RwLock<DeviceInfo>,
}

impl IdentityManager {
    /// Build an `IdentityManager` by:
    /// 1. Asking the keychain for (or generating) the Ed25519 keypair.
    /// 2. Deriving a libp2p PeerId from the public key.
    /// 3. Pulling device name / created_at from the loaded [`GlobalConfig`].
    /// 4. Filling host info (hostname / os / platform / arch) from the runtime.
    pub async fn new(
        keychain: Arc<dyn KeychainProvider>,
        config: &GlobalConfig,
    ) -> AppResult<Self> {
        let keypair_bytes = keychain.get_or_create_keypair().await?;
        let keypair = Keypair::from_protobuf_encoding(&keypair_bytes)
            .map_err(|e| AppError::KeypairDecode(e.to_string()))?;
        let peer_id = keypair.public().to_peer_id().to_string();

        let device_info = DeviceInfo {
            peer_id: peer_id.clone(),
            device_name: config.device_name.clone(),
            hostname: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_default(),
            os: std::env::consts::OS.to_string(),
            platform: std::env::consts::FAMILY.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            created_at: config.created_at.clone(),
        };

        info!("Device identity initialized: PeerId={}", peer_id);

        Ok(Self {
            keypair,
            device_info: RwLock::new(device_info),
        })
    }

    /// Run a closure against the held `DeviceInfo`. Read-only access.
    fn with_info<R>(&self, f: impl FnOnce(&DeviceInfo) -> R) -> AppResult<R> {
        let guard = self
            .device_info
            .read()
            .expect("device_info RwLock poisoned");
        Ok(f(&guard))
    }

    pub fn peer_id(&self) -> AppResult<String> {
        self.with_info(|info| info.peer_id.clone())
    }

    pub fn device_info(&self) -> AppResult<DeviceInfo> {
        self.with_info(|info| info.clone())
    }

    /// Update the user-facing device name in memory. Persistence is the
    /// caller's job (AppCore writes the backing `GlobalConfig` to disk).
    pub fn set_device_name(&self, name: String) -> AppResult<()> {
        let mut info = self
            .device_info
            .write()
            .expect("device_info RwLock poisoned");
        info.device_name = name;
        Ok(())
    }

    /// Serialize the held keypair into its libp2p protobuf encoding.
    /// Used by [`crate::AppCore::start_network`] to bootstrap the libp2p
    /// swarm without exposing the raw `Keypair` field.
    pub(crate) fn keypair_protobuf(&self) -> AppResult<Vec<u8>> {
        self.keypair
            .to_protobuf_encoding()
            .map_err(|e| AppError::KeypairEncode(e.to_string()))
    }

    /// This device's long-lived X25519 secret, derived from the Ed25519
    /// identity (single-layer reuse). Used as the sender/recipient key for
    /// workspace-key Lockboxes. See [`crate::crypto::keyx`].
    pub fn x25519_secret(&self) -> AppResult<x25519_dalek::StaticSecret> {
        let ed = self
            .keypair
            .clone()
            .try_into_ed25519()
            .map_err(|e| AppError::KeypairDecode(e.to_string()))?;
        let bytes = ed.to_bytes(); // [secret_seed(32) || public(32)]
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes[..32]);
        Ok(crate::crypto::keyx::derive_x25519_secret(&seed))
    }

    /// This device's X25519 public key (Lockbox recipient identity).
    pub fn x25519_public(&self) -> AppResult<x25519_dalek::PublicKey> {
        Ok(x25519_dalek::PublicKey::from(&self.x25519_secret()?))
    }

    /// Sign `msg` with this device's Ed25519 identity key. Used to sign
    /// permission operations so peers can verify the issuer.
    pub fn sign(&self, msg: &[u8]) -> AppResult<Vec<u8>> {
        self.keypair.sign(msg).map_err(|e| AppError::Crypto {
            context: "sign",
            reason: e.to_string(),
        })
    }
}

/// Recover a peer's libp2p Ed25519 [`PublicKey`] from its (inlined) PeerId.
fn peer_ed25519_public(
    peer_id: &swarm_p2p_core::libp2p::PeerId,
) -> AppResult<swarm_p2p_core::libp2p::identity::PublicKey> {
    use swarm_p2p_core::libp2p::identity::PublicKey;
    let mh = swarm_p2p_core::libp2p::multihash::Multihash::<64>::from_bytes(&peer_id.to_bytes())
        .map_err(|e| AppError::Crypto {
            context: "peer-pubkey",
            reason: e.to_string(),
        })?;
    if mh.code() != 0 {
        return Err(AppError::Crypto {
            context: "peer-pubkey",
            reason: "peer id is a hash, not an inlined key".into(),
        });
    }
    PublicKey::try_decode_protobuf(mh.digest()).map_err(|e| AppError::Crypto {
        context: "peer-pubkey",
        reason: e.to_string(),
    })
}

/// Derive a peer's X25519 public key from its (ed25519) PeerId. SwarmNote uses
/// ed25519 identities, whose public key is inlined in the PeerId's identity
/// multihash — so a Lockbox can be sealed to a paired device knowing only its
/// PeerId, with no extra key exchange. See [`crate::crypto::keyx`].
pub fn peer_id_to_x25519_public(
    peer_id: &swarm_p2p_core::libp2p::PeerId,
) -> AppResult<x25519_dalek::PublicKey> {
    let ed = peer_ed25519_public(peer_id)?
        .try_into_ed25519()
        .map_err(|e| AppError::Crypto {
            context: "peer-x25519",
            reason: e.to_string(),
        })?;
    crate::crypto::keyx::ed25519_pub_to_x25519(&ed.to_bytes())
}

/// Verify an Ed25519 signature against a peer's PeerId-derived public key.
/// Returns `false` on any decode/verify failure (never panics).
pub fn verify_peer_signature(
    peer_id: &swarm_p2p_core::libp2p::PeerId,
    msg: &[u8],
    sig: &[u8],
) -> bool {
    peer_ed25519_public(peer_id)
        .map(|pk| pk.verify(msg, sig))
        .unwrap_or(false)
}

#[cfg(test)]
mod x25519_tests {
    use super::*;
    use swarm_p2p_core::libp2p::identity::Keypair;

    #[test]
    fn peer_id_x25519_matches_direct_derivation() {
        let kp = Keypair::generate_ed25519();
        let peer_id = kp.public().to_peer_id();
        let ed = kp.try_into_ed25519().unwrap();
        let direct = crate::crypto::keyx::ed25519_pub_to_x25519(&ed.public().to_bytes()).unwrap();
        let via_peer = peer_id_to_x25519_public(&peer_id).unwrap();
        assert_eq!(direct.as_bytes(), via_peer.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InMemoryKeychain {
        stored: tokio::sync::Mutex<Option<Vec<u8>>>,
    }

    impl InMemoryKeychain {
        fn new() -> Self {
            Self {
                stored: tokio::sync::Mutex::new(None),
            }
        }
    }

    #[async_trait::async_trait]
    impl KeychainProvider for InMemoryKeychain {
        async fn get_or_create_keypair(&self) -> AppResult<Vec<u8>> {
            let mut guard = self.stored.lock().await;
            if let Some(bytes) = guard.as_ref() {
                return Ok(bytes.clone());
            }
            let kp = Keypair::generate_ed25519();
            let bytes = kp
                .to_protobuf_encoding()
                .map_err(|e| AppError::KeychainUnavailable(e.to_string()))?;
            *guard = Some(bytes.clone());
            Ok(bytes)
        }
    }

    fn test_config() -> GlobalConfig {
        GlobalConfig {
            device_name: "Test Device".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            last_workspace_path: None,
            recent_workspaces: Vec::new(),
        }
    }

    #[tokio::test]
    async fn first_launch_generates_identity() {
        let keychain = Arc::new(InMemoryKeychain::new());
        let config = test_config();
        let identity = IdentityManager::new(keychain, &config).await.unwrap();

        let peer_id = identity.peer_id().unwrap();
        assert!(
            peer_id.starts_with("12D3KooW"),
            "PeerId should start with 12D3KooW, got: {peer_id}"
        );
    }

    #[tokio::test]
    async fn restart_reuses_same_identity() {
        let keychain = Arc::new(InMemoryKeychain::new());
        let config = test_config();

        let id1 = IdentityManager::new(keychain.clone(), &config)
            .await
            .unwrap();
        let pid1 = id1.peer_id().unwrap();
        drop(id1);

        let id2 = IdentityManager::new(keychain, &config).await.unwrap();
        let pid2 = id2.peer_id().unwrap();

        assert_eq!(pid1, pid2);
    }

    #[tokio::test]
    async fn set_device_name_updates_snapshot() {
        let keychain = Arc::new(InMemoryKeychain::new());
        let config = test_config();
        let identity = IdentityManager::new(keychain, &config).await.unwrap();

        identity.set_device_name("My Laptop".to_string()).unwrap();
        assert_eq!(identity.device_info().unwrap().device_name, "My Laptop");
    }

    #[tokio::test]
    async fn device_info_contains_host_metadata() {
        let keychain = Arc::new(InMemoryKeychain::new());
        let config = test_config();
        let identity = IdentityManager::new(keychain, &config).await.unwrap();

        let info = identity.device_info().unwrap();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert_eq!(info.created_at, "2026-01-01T00:00:00Z");
    }
}
