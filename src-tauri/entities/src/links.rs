use sea_orm::{Linked, RelationTrait};

use crate::{component, structure};

pub struct ComponentOf;

impl Linked for ComponentOf {
    type FromEntity = structure::Entity;
    type ToEntity = component::Entity;

    fn link(&self) -> Vec<sea_orm::LinkDef> {
        vec![
            structure::Relation::ComponentOf.def()
        ]
    }
}

pub struct Components;

impl Linked for Components {
    type FromEntity = structure::Entity;
    type ToEntity = component::Entity;

    fn link(&self) -> Vec<sea_orm::LinkDef> {
        vec![
            structure::Relation::Components.def()
        ]
    }
}

pub struct ComponentStructure;

impl Linked for ComponentStructure {
    type FromEntity = component::Entity;
    type ToEntity = structure::Entity;
    
    fn link(&self) -> Vec<sea_orm::LinkDef> {
        vec![
            component::Relation::Component.def()
        ]
    }
}
