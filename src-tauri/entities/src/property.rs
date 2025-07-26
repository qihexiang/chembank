use serde::{Deserialize, Serialize};
use sea_orm::entity::prelude::*;
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, DeriveEntityModel, Type)]
#[sea_orm(table_name = "property")]
#[serde(rename = "Property")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub structure_id: u32,
    decomp_temp: Option<String>,
    density: Option<String>,
    diss_temp: Option<String>,
    formation_enthalpy: Option<String>,
    impact_sensitive: Option<String>,
    friction_sensitivity: Option<String>,
    det_velocity: Option<String>,
    det_pressure: Option<String>,
    n_content: Option<String>,
    o_content: Option<String>,
    no_content: Option<String>,
    references: Option<String>,
    remarks: Option<String>,
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