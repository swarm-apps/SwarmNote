use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workspaces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub created_by: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    #[sea_orm(has_many)]
    pub folders: HasMany<super::folders::Entity>,
    #[sea_orm(has_many)]
    pub documents: HasMany<super::documents::Entity>,
}

crate::impl_timestamped_behavior!(ActiveModel);
