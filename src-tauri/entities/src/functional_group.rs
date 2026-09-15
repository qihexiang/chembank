use serde::{Deserialize, Serialize};
use sea_orm::entity::prelude::*;
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, DeriveEntityModel, Type)]
#[sea_orm(table_name = "functional_groups")]
#[serde(rename = "FunctionalGroup")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: u32,
    #[sea_orm(unique)]
    name: String,
    smarts: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
