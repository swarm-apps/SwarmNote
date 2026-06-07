use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One node in the signed, append-only permission operation chain (auth DAG).
/// `op_id` is the content hash of the canonical op; `signature` is the issuer
/// device's Ed25519 signature over the op fields; `prev_hash` links the causal
/// predecessor (None for the genesis op). Replay validates signature + issuer
/// authority + anti-escalation before materializing into `permissions`.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "permission_ops")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub op_id: String,
    pub workspace_id: Uuid,
    pub op_kind: String,
    pub target_peer_id: String,
    pub new_role: String,
    pub issuer_peer_id: String,
    pub key_version: i32,
    pub prev_hash: Option<String>,
    pub signature: Vec<u8>,
    pub created_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
