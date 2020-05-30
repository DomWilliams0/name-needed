use crate::ecs::Entity;
use crate::item::{BaseItemComponent, ItemSlot};
use crate::ComponentWorld;

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug)]
pub enum ItemClass {
    Food,
    Weapon,
}

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug)]
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
