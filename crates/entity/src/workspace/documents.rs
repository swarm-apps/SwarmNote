use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "documents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub folder_id: Option<Uuid>,
    pub title: String,
    pub rel_path: String,
    pub file_hash: Option<Vec<u8>>,
    pub yjs_state: Option<Vec<u8>>,
    pub state_vector: Option<Vec<u8>>,
    pub lamport_clock: i64,
    pub created_by: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    #[sea_orm(belongs_to, from = "workspace_id", to = "id")]
    pub workspace: HasOne<super::workspaces::Entity>,
    #[sea_orm(belongs_to, from = "folder_id", to = "id")]
    pub folder: HasOne<super::folders::Entity>,
}

crate::impl_timestamped_behavior!(ActiveModel);
