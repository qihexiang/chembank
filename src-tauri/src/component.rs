use serde::{Deserialize, Serialize};
use sea_orm::entity::prelude::*;
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, DeriveEntityModel, Type)]
#[sea_orm(table_name = "components")]
#[serde(rename = "Component")]
pub struct Model {
    #[sea_orm(primary_key)]
    structure_id: u32,
    #[sea_orm(primary_key)]
    component_id: u32,
    #[sea_orm(default = 1)]
    count: u32
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::structure::Entity",
        from = "Column::StructureId",
        to = "super::structure::Column::Id"
    )]
    Structure,
    #[sea_orm(
        belongs_to = "super::structure::Entity",
        from = "Column::ComponentId",
        to = "super::structure::Column::Id"
    )]
    Component,
}

impl ActiveModelBehavior for ActiveModel {}