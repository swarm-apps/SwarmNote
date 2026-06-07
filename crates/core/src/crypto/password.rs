//! Argon2id KDF for optional link-share passwords.
//!
//! Parameters per RFC 9106 "second recommended": m = 64 MiB, t = 3, p = 4.
//! Derives a raw 32-byte key (not a PHC string) used to wrap the inner link DEK.

use argon2::{Algorithm, Argon2, Params, Version};

use super::KEY_LEN;
use crate::error::{AppError, AppResult};

fn err(reason: impl Into<String>) -> AppError {
    AppError::Crypto {
        context: "argon2",
        reason: reason.into(),
    }
}

/// Derive a 32-byte key from `password` + `salt` using Argon2id.
pub fn derive_password_key(password: &[u8], salt: &[u8]) -> AppResult<[u8; KEY_LEN]> {
    let params = Params::new(64 * 1024, 3, 4, Some(KEY_LEN)).map_err(|e| err(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut out)
        .map_err(|e| err(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_inputs() {
        let salt = b"0123456789abcdef";
        let a = derive_password_key(b"hunter2", salt).unwrap();
        let b = derive_password_key(b"hunter2", salt).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn salt_and_password_separate_outputs() {
        let k1 = derive_password_key(b"hunter2", b"0123456789abcdef").unwrap();
        let k2 = derive_password_key(b"hunter2", b"fedcba9876543210").unwrap();
        let k3 = derive_password_key(b"different", b"0123456789abcdef").unwrap();
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
    }
}
