use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "deletion_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub doc_id: Uuid,
    pub rel_path: String,
    pub deleted_at: DateTimeUtc,
    pub deleted_by: String,
    pub lamport_clock: i64,
}

impl ActiveModelBehavior for ActiveModel {}
