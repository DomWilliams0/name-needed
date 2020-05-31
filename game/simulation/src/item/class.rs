use crate::ecs::Entity;
use crate::item::{BaseItemComponent, ItemSlot};
use crate::{entity_pretty, ComponentWorld};
use std::fmt::{Display, Formatter};

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug, Ord, PartialOrd)]
pub enum ItemClass {
    Food,
    Weapon,
}

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug, Ord, PartialOrd)]
pub enum ItemFilter {
    Class(ItemClass),
    SpecificEntity(Entity),
    Predicate(fn(Entity) -> bool),
}

pub trait ItemFilterable {
    fn matches(self, filter: ItemFilter) -> Option<Entity>;
}

impl ItemFilterable for (Entity, &BaseItemComponent) {
    fn matches(self, filter: ItemFilter) -> Option<Entity> {
        let (entity, item) = self;
        if match filter {
            ItemFilter::Class(class) => item.class == class,
            ItemFilter::SpecificEntity(e) => e == entity,
            ItemFilter::Predicate(f) => f(entity),
        } {
            Some(entity)
        } else {
            None
        }
    }
}

impl<W: ComponentWorld> ItemFilterable for (&ItemSlot, Option<&W>) {
    /// `world` only needed for ItemClass filter
    fn matches(self, filter: ItemFilter) -> Option<Entity> {
        let (slot, world) = self;
        if let ItemSlot::Full(item) = slot {
            let found = match filter {
                ItemFilter::Class(class) => world
                    .as_ref()?
                    .component::<BaseItemComponent>(*item)
                    .map(|item| item.class == class)
                    .unwrap_or(false),
                ItemFilter::SpecificEntity(e) => e == *item,
                ItemFilter::Predicate(f) => f(*item),
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemFilter::Class(c) => write!(f, "item.class == {:?}", c),
            ItemFilter::SpecificEntity(e) => write!(f, "item == {}", entity_pretty!(e)),
            ItemFilter::Predicate(p) => write!(f, "f(item) where f = {:#x}", *p as usize),
        }
    }
}
