use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "doc_chunks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub doc_id: Uuid,
    pub chunk_offset: i32,
    pub chunk_length: i32,
    pub chunk_hash: Vec<u8>,
    #[sea_orm(belongs_to, from = "doc_id", to = "id")]
    pub document: HasOne<super::documents::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
