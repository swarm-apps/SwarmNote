//! Ed25519 → X25519 single-layer derivation + ECDH.
//!
//! Each device's long-lived X25519 keypair is derived from its Ed25519 identity
//! key (`SigningKey::to_scalar_bytes` for the secret, `VerifyingKey::to_montgomery`
//! for a peer's public). So a peer's X25519 public key can be computed from its
//! Ed25519 public key (≈ PeerId) alone — no extra key exchange at pairing time.
//! Clamp convention is fixed; we do **not** layer-derive. See 08-e2e-encryption.md.

use ed25519_dalek::{SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::error::{AppError, AppResult};

/// Derive this device's X25519 static secret from its Ed25519 seed (32 bytes).
pub fn derive_x25519_secret(ed25519_seed: &[u8; 32]) -> StaticSecret {
    let sk = SigningKey::from_bytes(ed25519_seed);
    StaticSecret::from(sk.to_scalar_bytes())
}

/// Compute a peer's X25519 public key from its Ed25519 public key (32 bytes).
pub fn ed25519_pub_to_x25519(ed25519_pub: &[u8; 32]) -> AppResult<PublicKey> {
    let vk = VerifyingKey::from_bytes(ed25519_pub).map_err(|e| AppError::Crypto {
        context: "keyx",
        reason: format!("bad ed25519 public key: {e}"),
    })?;
    Ok(PublicKey::from(vk.to_montgomery().to_bytes()))
}

/// X25519 Diffie-Hellman shared secret (both keys are device-static).
pub fn x25519_dh(my_secret: &StaticSecret, their_public: &PublicKey) -> [u8; 32] {
    my_secret.diffie_hellman(their_public).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed_pub(seed: &[u8; 32]) -> [u8; 32] {
        SigningKey::from_bytes(seed).verifying_key().to_bytes()
    }

    #[test]
    fn cross_device_dh_agrees() {
        // Two devices, identities derived from distinct Ed25519 seeds.
        let seed_a = [11u8; 32];
        let seed_b = [22u8; 32];

        let sec_a = derive_x25519_secret(&seed_a);
        let sec_b = derive_x25519_secret(&seed_b);

        // Each computes the OTHER's X25519 public from its Ed25519 public only.
        let pub_a = ed25519_pub_to_x25519(&ed_pub(&seed_a)).unwrap();
        let pub_b = ed25519_pub_to_x25519(&ed_pub(&seed_b)).unwrap();

        let ab = x25519_dh(&sec_a, &pub_b);
        let ba = x25519_dh(&sec_b, &pub_a);
        assert_eq!(ab, ba, "Ed25519→X25519 DH must agree across devices");
    }

    #[test]
    fn derived_public_matches_secret() {
        let seed = [33u8; 32];
        let sec = derive_x25519_secret(&seed);
        let from_secret = PublicKey::from(&sec);
        let from_ed = ed25519_pub_to_x25519(&ed_pub(&seed)).unwrap();
        assert_eq!(from_secret.as_bytes(), from_ed.as_bytes());
    }

    #[test]
    fn distinct_seeds_distinct_secrets() {
        // Sanity: different device identities yield different X25519 keys.
        let a = derive_x25519_secret(&[1u8; 32]);
        let b = derive_x25519_secret(&[2u8; 32]);
        assert_ne!(a.to_bytes(), b.to_bytes());
    }
}
