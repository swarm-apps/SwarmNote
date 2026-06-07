//! XChaCha20-Poly1305 framed, key-committing seal/open.
//!
//! Frame: `[1B version][4B key_version BE][24B nonce][32B commitment][ciphertext]`.
//! The commitment binds the frame to the workspace master key; it is verified
//! (constant-time) before AEAD decryption so a ciphertext forged under another
//! key is rejected up front.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use subtle::ConstantTimeEq;

use super::kdf::{derive_commitment, derive_subkey};
use super::{fill_random, Purpose, COMMITMENT_LEN, KEY_LEN, NONCE_LEN};
use crate::error::{AppError, AppResult};

const FRAME_VERSION: u8 = 1;
const HEADER_LEN: usize = 1 + 4 + NONCE_LEN + COMMITMENT_LEN; // 61

fn err(reason: impl Into<String>) -> AppError {
    AppError::Crypto {
        context: "aead",
        reason: reason.into(),
    }
}

/// Encrypt `plaintext` under a subkey derived from `master` for `purpose`,
/// producing a framed key-committing ciphertext.
pub fn seal(
    master: &[u8; KEY_LEN],
    purpose: Purpose,
    workspace_id: &[u8; 16],
    key_version: u32,
    aad: &[u8],
    plaintext: &[u8],
) -> AppResult<Vec<u8>> {
    let subkey = derive_subkey(master, purpose, workspace_id, key_version);
    let commitment = derive_commitment(master, workspace_id, key_version);

    let cipher = XChaCha20Poly1305::new_from_slice(&subkey).map_err(|_| err("bad key length"))?;
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce);

    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| err("encrypt failed"))?;

    let mut frame = Vec::with_capacity(HEADER_LEN + ct.len());
    frame.push(FRAME_VERSION);
    frame.extend_from_slice(&key_version.to_be_bytes());
    frame.extend_from_slice(&nonce);
    frame.extend_from_slice(&commitment);
    frame.extend_from_slice(&ct);
    Ok(frame)
}

/// Read the `key_version` from a frame header (cheap — lets the receiver pick
/// the right master key from its key history before [`open`]).
pub fn frame_key_version(frame: &[u8]) -> AppResult<u32> {
    if frame.len() < HEADER_LEN || frame[0] != FRAME_VERSION {
        return Err(err("bad frame header"));
    }
    Ok(u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]))
}

/// Decrypt a framed ciphertext. Verifies the frame's `key_version` matches and
/// the key commitment matches `master` (constant-time) before AEAD decryption.
pub fn open(
    master: &[u8; KEY_LEN],
    purpose: Purpose,
    workspace_id: &[u8; 16],
    key_version: u32,
    aad: &[u8],
    frame: &[u8],
) -> AppResult<Vec<u8>> {
    if frame.len() < HEADER_LEN || frame[0] != FRAME_VERSION {
        return Err(err("bad frame header"));
    }
    let fv = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]);
    if fv != key_version {
        return Err(err("key_version mismatch"));
    }
    let nonce = &frame[5..5 + NONCE_LEN];
    let commitment = &frame[5 + NONCE_LEN..HEADER_LEN];
    let ct = &frame[HEADER_LEN..];

    let expected = derive_commitment(master, workspace_id, key_version);
    if expected.ct_eq(commitment).unwrap_u8() != 1 {
        return Err(err("key commitment mismatch"));
    }

    let subkey = derive_subkey(master, purpose, workspace_id, key_version);
    let cipher = XChaCha20Poly1305::new_from_slice(&subkey).map_err(|_| err("bad key length"))?;
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
        .map_err(|_| err("decrypt/authenticate failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WS: [u8; 16] = [9u8; 16];
    const AAD: &[u8] = b"workspace||doc||1||ws";

    #[test]
    fn round_trip() {
        let key = [3u8; 32];
        let msg = b"hello swarm \xe4\xbd\xa0\xe5\xa5\xbd"; // includes CJK bytes
        let frame = seal(&key, Purpose::Gossip, &WS, 1, AAD, msg).unwrap();
        assert_eq!(frame_key_version(&frame).unwrap(), 1);
        let out = open(&key, Purpose::Gossip, &WS, 1, AAD, &frame).unwrap();
        assert_eq!(out, msg);
    }

    #[test]
    fn wrong_master_rejected_by_commitment() {
        let frame = seal(&[3u8; 32], Purpose::Gossip, &WS, 1, AAD, b"x").unwrap();
        let e = open(&[4u8; 32], Purpose::Gossip, &WS, 1, AAD, &frame).unwrap_err();
        assert!(matches!(e, AppError::Crypto { .. }));
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let key = [3u8; 32];
        let mut frame = seal(&key, Purpose::Gossip, &WS, 1, AAD, b"payload").unwrap();
        let last = frame.len() - 1;
        frame[last] ^= 0xff;
        assert!(open(&key, Purpose::Gossip, &WS, 1, AAD, &frame).is_err());
    }

    #[test]
    fn aad_mismatch_rejected() {
        let key = [3u8; 32];
        let frame = seal(&key, Purpose::Gossip, &WS, 1, AAD, b"payload").unwrap();
        assert!(open(&key, Purpose::Gossip, &WS, 1, b"other-aad", &frame).is_err());
    }

    #[test]
    fn key_version_mismatch_rejected() {
        let key = [3u8; 32];
        let frame = seal(&key, Purpose::Gossip, &WS, 1, AAD, b"payload").unwrap();
        assert!(open(&key, Purpose::Gossip, &WS, 2, AAD, &frame).is_err());
    }

    #[test]
    fn large_payload() {
        let key = [5u8; 32];
        let msg = vec![0xabu8; 256 * 1024];
        let frame = seal(&key, Purpose::Asset, &WS, 7, AAD, &msg).unwrap();
        assert_eq!(
            open(&key, Purpose::Asset, &WS, 7, AAD, &frame).unwrap(),
            msg
        );
    }
}
