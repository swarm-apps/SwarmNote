//! X25519 sealed key envelope ("Lockbox").
//!
//! Distributes a workspace key bundle to a recipient device's X25519 public key.
//! Frame: `[1B version][24B nonce][32B commitment][ciphertext]`.
//! Shared secret = X25519(my_secret, their_public); both keys are device-static,
//! so the recipient authenticates the sender by deriving with the sender's
//! public key. KEK + commitment are HKDF-derived from the shared secret.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use x25519_dalek::{PublicKey, StaticSecret};

use super::keyx::x25519_dh;
use super::{fill_random, Purpose, COMMITMENT_LEN, KEY_LEN, NONCE_LEN};
use crate::error::{AppError, AppResult};

const FRAME_VERSION: u8 = 1;
const HEADER_LEN: usize = 1 + NONCE_LEN + COMMITMENT_LEN; // 57
const AAD: &[u8] = b"swarmnote:v1:lockbox";

fn err(reason: impl Into<String>) -> AppError {
    AppError::Crypto {
        context: "lockbox",
        reason: reason.into(),
    }
}

fn derive(shared: &[u8; 32], purpose: Purpose) -> [u8; KEY_LEN] {
    let hk = Hkdf::<Sha256>::new(None, shared);
    let mut out = [0u8; KEY_LEN];
    hk.expand(purpose.label(), &mut out)
        .expect("HKDF expand of 32 bytes never fails");
    out
}

/// Seal `plaintext` (a workspace key bundle) for `recipient_public`.
pub fn seal_lockbox(
    my_secret: &StaticSecret,
    recipient_public: &PublicKey,
    plaintext: &[u8],
) -> AppResult<Vec<u8>> {
    let shared = x25519_dh(my_secret, recipient_public);
    let kek = derive(&shared, Purpose::Kek);
    let commitment = derive(&shared, Purpose::Commit);

    let cipher = XChaCha20Poly1305::new_from_slice(&kek).map_err(|_| err("bad kek length"))?;
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce);
    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| err("encrypt failed"))?;

    let mut frame = Vec::with_capacity(HEADER_LEN + ct.len());
    frame.push(FRAME_VERSION);
    frame.extend_from_slice(&nonce);
    frame.extend_from_slice(&commitment[..COMMITMENT_LEN]);
    frame.extend_from_slice(&ct);
    Ok(frame)
}

/// Open a Lockbox sealed by `sender_public` for this device's `my_secret`.
pub fn open_lockbox(
    my_secret: &StaticSecret,
    sender_public: &PublicKey,
    frame: &[u8],
) -> AppResult<Vec<u8>> {
    if frame.len() < HEADER_LEN || frame[0] != FRAME_VERSION {
        return Err(err("bad frame header"));
    }
    let shared = x25519_dh(my_secret, sender_public);
    let nonce = &frame[1..1 + NONCE_LEN];
    let commitment = &frame[1 + NONCE_LEN..HEADER_LEN];
    let ct = &frame[HEADER_LEN..];

    let expected = derive(&shared, Purpose::Commit);
    if expected[..COMMITMENT_LEN].ct_eq(commitment).unwrap_u8() != 1 {
        return Err(err("key commitment mismatch"));
    }
    let kek = derive(&shared, Purpose::Kek);
    let cipher = XChaCha20Poly1305::new_from_slice(&kek).map_err(|_| err("bad kek length"))?;
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad: AAD })
        .map_err(|_| err("decrypt/authenticate failed"))
}

#[cfg(test)]
mod tests {
    use super::super::keyx::derive_x25519_secret;
    use super::*;

    fn device(seed: u8) -> (StaticSecret, PublicKey) {
        let sec = derive_x25519_secret(&[seed; 32]);
        let pubk = PublicKey::from(&sec);
        (sec, pubk)
    }

    #[test]
    fn round_trip() {
        let (a_sec, a_pub) = device(1);
        let (b_sec, b_pub) = device(2);
        let bundle = b"read_key||write_key||key_version=1";

        let lb = seal_lockbox(&a_sec, &b_pub, bundle).unwrap();
        let out = open_lockbox(&b_sec, &a_pub, &lb).unwrap();
        assert_eq!(out, bundle);
    }

    #[test]
    fn wrong_recipient_rejected() {
        let (a_sec, _a_pub) = device(1);
        let (_b_sec, b_pub) = device(2);
        let (c_sec, _c_pub) = device(3);

        let lb = seal_lockbox(&a_sec, &b_pub, b"secret").unwrap();
        // Device C (not the recipient) cannot open it.
        let a_pub = PublicKey::from(&a_sec);
        assert!(open_lockbox(&c_sec, &a_pub, &lb).is_err());
    }
}
