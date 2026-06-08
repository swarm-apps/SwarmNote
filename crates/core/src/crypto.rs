//! Cryptography for E2E workspace sharing (v1).
//!
//! **Transit-only**: per-workspace symmetric keys encrypt GossipSub broadcasts;
//! X25519 Lockboxes distribute those keys to per-device public keys (each
//! derived from the device's Ed25519 identity). Authorized devices still write
//! plaintext `.md` locally (folder-is-truth). See
//! `dev-notes/design/08-e2e-encryption.md` and `11-threat-model.md`.
//!
//! Submodules:
//! * [`kdf`] — HKDF-SHA256 subkey derivation + key commitment (domain-separated)
//! * [`aead`] — XChaCha20-Poly1305 framed, key-committing seal/open
//! * [`keyx`] — Ed25519 → X25519 single-layer derivation + ECDH
//! * [`lockbox`] — X25519 sealed key envelope
//! * [`password`] — Argon2id link-password KDF
//!
//! All randomness comes from the OS-seeded CSPRNG via [`fill_random`]; we never
//! feed an RNG into the dalek APIs (avoids `rand_core` version coupling).

pub mod aead;
pub mod kdf;
pub mod keyx;
pub mod lockbox;
pub mod password;

use rand::RngCore;

/// Symmetric key size (read_key / write_key / derived subkeys).
pub const KEY_LEN: usize = 32;
/// XChaCha20-Poly1305 nonce length (192-bit → safe random nonces, no counter).
pub const NONCE_LEN: usize = 24;
/// Key-commitment length.
pub const COMMITMENT_LEN: usize = 32;

/// HKDF `info` purposes — domain-separate subkeys derived from one master key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Encrypt gossip doc-update / awareness broadcasts.
    Gossip,
    /// Encrypt asset chunk broadcasts.
    Asset,
    /// Key-commitment value.
    Commit,
    /// Lockbox key-encryption key.
    Kek,
}

impl Purpose {
    pub(crate) const fn label(self) -> &'static [u8] {
        match self {
            Purpose::Gossip => b"swarmnote:v1:gossip",
            Purpose::Asset => b"swarmnote:v1:asset",
            Purpose::Commit => b"swarmnote:v1:commit",
            Purpose::Kek => b"swarmnote:v1:kek",
        }
    }
}

/// Fill `buf` with CSPRNG bytes (OS-seeded, periodically reseeding thread RNG).
pub fn fill_random(buf: &mut [u8]) {
    rand::rng().fill_bytes(buf);
}

/// Generate a fresh 32-byte symmetric key.
pub fn random_key() -> [u8; KEY_LEN] {
    let mut k = [0u8; KEY_LEN];
    fill_random(&mut k);
    k
}

/// Generate `N` fresh CSPRNG bytes (nonces, salts, link secrets).
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    fill_random(&mut b);
    b
}

// Ergonomic re-exports — callers use `crypto::seal`, `crypto::seal_lockbox`, …
pub use aead::{frame_key_version, open, seal};
pub use kdf::{derive_commitment, derive_subkey};
pub use keyx::{derive_x25519_secret, ed25519_pub_to_x25519, x25519_dh};
pub use lockbox::{open_lockbox, seal_lockbox};
pub use password::derive_password_key;
