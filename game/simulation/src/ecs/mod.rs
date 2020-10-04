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

pub struct E(pub Entity);

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
}
