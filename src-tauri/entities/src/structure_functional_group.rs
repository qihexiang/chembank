use serde::{Deserialize, Serialize};
use sea_orm::entity::prelude::*;
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, DeriveEntityModel, Type)]
#[sea_orm(table_name = "structure_functional_groups")]
#[serde(rename = "StructureFunctionalGroup")]
pub struct Model {
    #[sea_orm(primary_key)]
    structure_id: u32,
    #[sea_orm(primary_key)]
    functional_group_id: u32,
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
        belongs_to = "super::functional_group::Entity",
        from = "Column::FunctionalGroupId",
        to = "super::functional_group::Column::Id"
    )]
    FunctionalGroup,
}

impl ActiveModelBehavior for ActiveModel {}
