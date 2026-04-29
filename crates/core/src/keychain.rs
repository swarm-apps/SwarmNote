//! Platform-abstracted secret storage for the device's Ed25519 keypair.

use async_trait::async_trait;

use crate::error::AppResult;

/// Platform keychain / keystore abstraction. Only handles the long-lived
/// device identity keypair — device names, hostnames, and OS metadata are
/// plaintext (stored in config files) and NOT part of this interface.
#[async_trait]
pub trait KeychainProvider: Send + Sync + 'static {
    /// Return the protobuf-encoded Ed25519 keypair bytes, generating a new
    /// keypair on first call if none exists.
    ///
    /// Encoding MUST be libp2p-compatible (`Keypair::to_protobuf_encoding` /
    /// `Keypair::from_protobuf_encoding`) so desktop (`keyring` crate) and
    /// mobile (Android Keystore / iOS Keychain) impls produce an interoperable
    /// byte representation.
    async fn get_or_create_keypair(&self) -> AppResult<Vec<u8>>;
}
