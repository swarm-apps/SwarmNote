use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workspace_keys")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: Uuid,
    pub read_key_enc: Vec<u8>,
    pub write_key_enc: Option<Vec<u8>>,
    pub admin_key_enc: Option<Vec<u8>>,
    pub key_version: i32,
    pub updated_at: DateTimeUtc,
    #[sea_orm(belongs_to, from = "workspace_id", to = "id")]
    pub workspace: HasOne<super::workspaces::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
