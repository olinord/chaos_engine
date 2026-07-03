use std::any::TypeId;

use crate::ecs::{EntityID, LookupID};

#[derive(Clone, PartialEq, Debug)]
pub enum ComponentErrors {
    EntityNotFound(EntityID),
    ComponentNorRegistered(String),
    ComponentNotFound(TypeId),
    ComponentNotFoundForEntity(String, EntityID),
    DuplicateComponent(TypeId),
    ComponentCastError(TypeId),
    ComponentLookupNotFound(LookupID),
    AddComponentMessageNotSent(String),
    RemoveComponentMessageNotSent(String),
}
