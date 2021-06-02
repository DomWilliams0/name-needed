use crate::ecs::Entity;
use crate::ComponentWorld;
use common::*;

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug, Ord, PartialOrd)]
pub enum ItemFilter {
    SpecificEntity(Entity),
    Predicate(fn(Entity) -> bool),
    HasComponent(&'static str),
    // TODO filters on other fields e.g. mass, size, condition, etc
}

pub trait ItemFilterable {
    /// Panics if world is None and filter requires it, only use None if the filter cannot possibly
    /// need it
    fn matches(self, filter: ItemFilter) -> bool;
}

impl<W: ComponentWorld> ItemFilterable for (Entity, Option<&W>) {
    fn matches(self, filter: ItemFilter) -> bool {
        let (item, world) = self;
        match filter {
            ItemFilter::SpecificEntity(e) => {
                let matches = e == item;

                if cfg!(debug_assertions) {
                    if let Some(world) = world {
                        use crate::item::ContainedInComponent;
                        assert!(world.has_component::<ContainedInComponent>(item));
                    }
                }

                matches
            }
            ItemFilter::Predicate(f) => f(item),
            ItemFilter::HasComponent(comp) => world.unwrap().has_component_by_name(comp, item),
        }
    }
}

impl<W: ComponentWorld> ItemFilterable for (Option<Entity>, Option<&W>) {
    fn matches(self, filter: ItemFilter) -> bool {
        let (item, world) = self;
        if let Some(item) = item {
            (item, world).matches(filter)
        } else {
            false
        }
    }
}

impl Display for ItemFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ItemFilter::SpecificEntity(e) => write!(f, "item == {}", e),
            ItemFilter::Predicate(p) => write!(f, "f(item) where f = {:#x}", *p as usize),
            ItemFilter::HasComponent(comp) => write!(f, "item has {:?}", comp),
        }
    }
}
