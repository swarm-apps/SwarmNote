use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One per-device key Lockbox: the workspace `read_key` (and, for writers,
/// `write_key`) of a given `key_version`, sealed to a recipient device's X25519
/// public key. `sealed_*` blobs are self-contained Lockbox frames (nonce +
/// commitment + ciphertext). A read-only recipient has `sealed_write_key = None`.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workspace_key_lockboxes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub key_version: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub recipient_peer_id: String,
    pub sealed_read_key: Vec<u8>,
    pub sealed_write_key: Option<Vec<u8>>,
    pub sealed_by_peer_id: String,
    pub created_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
