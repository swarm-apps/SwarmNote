use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "permissions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub resource_type: String,
    pub resource_id: String,
    pub peer_id: String,
    pub role: String,
    pub granted_by: String,
    pub granted_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
