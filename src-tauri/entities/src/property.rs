use serde::{Deserialize, Serialize};
use sea_orm::entity::prelude::*;
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, DeriveEntityModel, Type)]
#[sea_orm(table_name = "property")]
#[serde(rename = "Property")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub structure_id: u32,
    decomp_temp: Option<f64>,
    density: Option<f64>,
    diss_temp: Option<f64>,
    formation_enthalpy: Option<f64>,
    impact_sensitive: Option<f64>,
    friction_sensitivity: Option<f64>,
    det_velocity: Option<f64>,
    det_pressure: Option<f64>,
    n_content: Option<f64>,
    o_content: Option<f64>,
    no_content: Option<f64>,
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