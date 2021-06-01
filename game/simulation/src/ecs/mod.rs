use std::collections::HashMap;

pub use specs::{
    world::EntitiesRes, Builder, Component, DenseVecStorage, Entity, EntityBuilder, HashMapStorage,
    Join, LazyUpdate, NullStorage, Read, ReadExpect, ReadStorage, RunNow, System, SystemData,
    VecStorage, WorldExt, Write, WriteExpect, WriteStorage,
};
pub use specs_derive::Component;

use common::*;
pub use component::{ComponentBuildError, Map, Value};
pub use ecs_derive::EcsComponent;
pub use template::{ComponentTemplate, ComponentTemplateEntry, ValueImpl};

pub use crate::register_component_template;

pub use self::world::{ComponentGetError, ComponentWorld, EcsWorld, EcsWorldFrameRef};
pub type SpecsWorld = specs::World;

mod component;
mod template;
mod world;
mod world_ext;

/// Displayable wrapper around spec's entity
pub struct E(pub Entity);

/// Copy of spec's entity type.
///
/// It's technically undefined to transmute like this but there's a unit test to confirm it's valid.
/// We might eventually reimplement the ECS ourselves too
#[derive(Copy, Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct EntityWrapper(pub specs::world::Index, pub std::num::NonZeroI32);

mod entity_fmt {
    use common::*;

    use super::E;

    impl slog::KV for E {
        fn serialize(&self, _: &Record, serializer: &mut dyn Serializer) -> SlogResult<()> {
            serializer.emit_arguments("entity", &format_args!("{}", self))
        }
    }

    impl slog::Value for E {
        fn serialize(
            &self,
            _: &Record,
            key: Key,
            serializer: &mut dyn Serializer,
        ) -> SlogResult<()> {
            serializer.emit_arguments(key, &format_args!("{}", self))
        }
    }

    impl Display for E {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "E{}", crate::entity_pretty!(self.0))
        }
    }
}

pub type HasCompFn = fn(&EcsWorld, Entity) -> bool;
pub type RegisterCompFn = fn(&mut SpecsWorld);

pub struct ComponentEntry {
    pub name: &'static str,
    pub has_comp_fn: HasCompFn,
    pub register_comp_fn: RegisterCompFn,
}

inventory::collect!(ComponentEntry);

pub struct ComponentRegistry {
    // TODO perfect hashing
    map: HashMap<&'static str, ComponentFunctions>,
}

struct ComponentFunctions {
    has_comp: HasCompFn,
}

impl ComponentRegistry {
    pub fn new(world: &mut SpecsWorld) -> Self {
        let mut map = HashMap::with_capacity(128);
        for comp in inventory::iter::<ComponentEntry> {
            debug!("registering component {:?}", comp.name);
            let old = map.insert(
                comp.name,
                ComponentFunctions {
                    has_comp: comp.has_comp_fn,
                },
            );

            if old.is_some() {
                panic!("duplicate component with name {:?}", comp.name)
            }

            (comp.register_comp_fn)(world);
        }

        info!("registered {} components", map.len());
        map.shrink_to_fit();
        ComponentRegistry { map }
    }

    pub fn has_component(&self, comp: &str, world: &EcsWorld, entity: Entity) -> bool {
        match self.map.get(comp) {
            Some(funcs) => (funcs.has_comp)(world, entity),
            None => {
                warn!("looking up non-existent component {:?}", comp);
                if cfg!(debug_assertions) {
                    panic!("looking up non-existent component {:?}", comp)
                }
                false
            }
        }
    }

    /// Iterates through all known component types and checks each one
    pub fn all_components_for<'a>(
        &'a self,
        world: &'a EcsWorld,
        entity: Entity,
    ) -> impl Iterator<Item = &'static str> + 'a {
        self.map.iter().filter_map(move |(name, funcs)| {
            if (funcs.has_comp)(world, entity) {
                Some(*name)
            } else {
                None
            }
        })
    }
}

impl From<EntityWrapper> for Entity {
    fn from(e: EntityWrapper) -> Self {
        // safety: see doc comment on EntityWrapper (and unit test below)
        unsafe { std::mem::transmute(e) }
    }
}

impl Display for EntityWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", E(Entity::from(*self)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroI32;

    #[test]
    fn entity_id_conversion() {
        let mut world = specs::World::new();

        for _ in 0..10 {
            let e = world.create_entity().build();

            let index = e.id();
            let gen = e.gen();

            let my_e = EntityWrapper(index, NonZeroI32::new(gen.id()).unwrap());
            let my_e = specs::Entity::from(my_e);
            assert_eq!(e, my_e, "specs entity layout has changed")
        }
    }
}
