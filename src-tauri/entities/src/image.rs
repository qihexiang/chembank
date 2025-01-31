use serde::{Deserialize, Serialize};
use sea_orm::entity::prelude::*;
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, DeriveEntityModel, Type)]
#[sea_orm(table_name = "images")]
#[serde(rename = "Image")]
pub struct Model {
    #[sea_orm(primary_key)]
    structure_id: u32,
    #[sea_orm(not_null)]
    extension: String,
    #[sea_orm(not_null)]
    image: Vec<u8>
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::structure::Entity",
        from = "Column::StructureId",
        to = "super::structure::Column::Id"
    )]
    Structure
}

impl Related<super::structure::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Structure.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}