use crate::ecs::Entity;
use crate::item::{BaseItemComponent, ItemSlot};
use crate::{entity_pretty, ComponentWorld};
use common::*;

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug, Ord, PartialOrd)]
pub enum ItemFilter {
    SpecificEntity(Entity),
    Predicate(fn(Entity) -> bool),
    HasComponent(&'static str),
    // TODO filters on item fields e.g. mass, slots, etc
}

pub trait ItemFilterable {
    fn matches(self, filter: ItemFilter) -> Option<Entity>;
}

impl<W: ComponentWorld> ItemFilterable for (Entity, &BaseItemComponent, Option<&W>) {
    /// Panics if world is None and filter requires it, only use None if the filter cannot possibly
    /// need it
    fn matches(self, filter: ItemFilter) -> Option<Entity> {
        let (entity, _, world) = self;
        if match filter {
            ItemFilter::SpecificEntity(e) => e == entity,
            ItemFilter::Predicate(f) => f(entity),
            ItemFilter::HasComponent(comp) => world.unwrap().has_component_by_name(comp, entity),
        } {
            Some(entity)
        } else {
            None
        }
    }
}

impl<W: ComponentWorld> ItemFilterable for (&ItemSlot, Option<&W>) {
    /// Panics if world is None and filter requires it, only use None if the filter cannot possibly
    /// need it
    fn matches(self, filter: ItemFilter) -> Option<Entity> {
        let (slot, world) = self;
        if let ItemSlot::Full(item) = slot {
            let found = match filter {
                ItemFilter::SpecificEntity(e) => e == *item,
                ItemFilter::Predicate(f) => f(*item),
                ItemFilter::HasComponent(comp) => world.unwrap().has_component_by_name(comp, *item),
            };
            if found {
                Some(*item)
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Display for ItemFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ItemFilter::SpecificEntity(e) => write!(f, "item == {}", entity_pretty!(e)),
            ItemFilter::Predicate(p) => write!(f, "f(item) where f = {:#x}", *p as usize),
            ItemFilter::HasComponent(comp) => write!(f, "item has {:?}", comp),
        }
    }
}
