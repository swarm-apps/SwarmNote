use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "share_invites")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub token: String,
    pub resource_type: String,
    pub resource_id: String,
    pub role: String,
    pub encrypted_keys: Vec<u8>,
    pub created_by: String,
    pub created_at: DateTimeUtc,
    pub expires_at: DateTimeUtc,
    pub max_uses: Option<i32>,
    #[sea_orm(default_value = 0)]
    pub used_count: i32,
    pub password_hash: Option<String>,
    /// Key commitment over the secret/password-wrapped invite (partitioning-
    /// oracle defense). Nullable for rows created before this column existed.
    pub commitment: Option<Vec<u8>>,
}

impl ActiveModelBehavior for ActiveModel {}
