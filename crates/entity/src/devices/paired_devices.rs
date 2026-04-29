use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "paired_devices")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub peer_id: String,
    pub name: Option<String>,
    pub hostname: String,
    pub os: Option<String>,
    pub platform: Option<String>,
    pub arch: Option<String>,
    pub paired_at: DateTimeUtc,
    pub last_seen: Option<DateTimeUtc>,
}

impl ActiveModelBehavior for ActiveModel {}
