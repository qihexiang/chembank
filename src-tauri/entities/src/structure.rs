use serde::{Deserialize, Serialize};
use sea_orm::entity::prelude::*;
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, DeriveEntityModel, Type)]
#[sea_orm(table_name = "structures")]
#[serde(rename = "Structure")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: u32,
    #[sea_orm(unique)]
    name: Option<String>,
    formula: String,
    #[sea_orm(unique)]
    smiles: Option<String>,
    #[sea_orm(not_null, default)]
    charge: i8,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::image::Entity")]
    Image,
    #[sea_orm(has_one = "super::property::Entity")]
    Property,
    #[sea_orm(belongs_to = "super::component::Entity", from = "Column::Id", to = "super::component::Column::StructureId")]
    Components,
    #[sea_orm(belongs_to = "super::component::Entity", from = "Column::Id", to = "super::component::Column::ComponentId")]
    ComponentOf
}

impl Related<super::image::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Image.def()
    }
}

impl Related<super::property::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Property.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
