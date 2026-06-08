//! HKDF-SHA256 subkey derivation + key commitment.
//!
//! `info = purpose_label || workspace_id(16) || key_version(4, big-endian)`,
//! `salt = workspace_id` (non-secret, stable). Domain separation ensures the
//! gossip key, asset key and commitment derived from one `read_key` are
//! cryptographically independent.

use hkdf::Hkdf;
use sha2::Sha256;

use super::{Purpose, COMMITMENT_LEN, KEY_LEN};

fn info(purpose: Purpose, workspace_id: &[u8; 16], key_version: u32) -> Vec<u8> {
    let label = purpose.label();
    let mut v = Vec::with_capacity(label.len() + 16 + 4);
    v.extend_from_slice(label);
    v.extend_from_slice(workspace_id);
    v.extend_from_slice(&key_version.to_be_bytes());
    v
}

/// Derive a 32-byte subkey from `master` for `purpose` in the given context.
pub fn derive_subkey(
    master: &[u8; KEY_LEN],
    purpose: Purpose,
    workspace_id: &[u8; 16],
    key_version: u32,
) -> [u8; KEY_LEN] {
    let hk = Hkdf::<Sha256>::new(Some(workspace_id), master);
    let mut out = [0u8; KEY_LEN];
    hk.expand(&info(purpose, workspace_id, key_version), &mut out)
        .expect("HKDF expand of 32 bytes never fails");
    out
}

/// Derive the key-commitment value binding a ciphertext to `master`.
/// Verified (constant-time) on decrypt to reject ciphertexts made under a
/// different master key (partitioning-oracle / invisible-salamander defense).
pub fn derive_commitment(
    master: &[u8; KEY_LEN],
    workspace_id: &[u8; 16],
    key_version: u32,
) -> [u8; COMMITMENT_LEN] {
    let hk = Hkdf::<Sha256>::new(Some(workspace_id), master);
    let mut out = [0u8; COMMITMENT_LEN];
    hk.expand(&info(Purpose::Commit, workspace_id, key_version), &mut out)
        .expect("HKDF expand of 32 bytes never fails");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let m = [7u8; 32];
        let ws = [1u8; 16];
        assert_eq!(
            derive_subkey(&m, Purpose::Gossip, &ws, 1),
            derive_subkey(&m, Purpose::Gossip, &ws, 1)
        );
    }

    #[test]
    fn domain_separated() {
        let m = [7u8; 32];
        let ws = [1u8; 16];
        let gossip = derive_subkey(&m, Purpose::Gossip, &ws, 1);
        let asset = derive_subkey(&m, Purpose::Asset, &ws, 1);
        let next_ver = derive_subkey(&m, Purpose::Gossip, &ws, 2);
        let commit = derive_commitment(&m, &ws, 1);
        assert_ne!(gossip, asset);
        assert_ne!(gossip, next_ver);
        assert_ne!(gossip, commit);
    }

    #[test]
    fn commitment_changes_with_master() {
        let ws = [1u8; 16];
        assert_ne!(
            derive_commitment(&[1u8; 32], &ws, 1),
            derive_commitment(&[2u8; 32], &ws, 1)
        );
    }
}
