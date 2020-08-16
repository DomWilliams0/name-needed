mod component;
mod template;
mod world;

pub use specs::{
    world::EntitiesRes, Component, DenseVecStorage, Entity, HashMapStorage, Join, LazyUpdate,
    NullStorage, Read, ReadExpect, ReadStorage, System, SystemData, VecStorage, WorldExt, Write,
    WriteExpect, WriteStorage,
};
pub use specs_derive::Component;

pub use self::world::{ComponentGetError, ComponentWorld, EcsWorld, EcsWorldFrameRef};
pub use crate::register_component_template;
pub use component::{ComponentBuildError, Map, Value};
pub use template::{ComponentTemplate, ComponentTemplateEntry, ValueImpl};

pub struct E(pub Entity);

mod entity_fmt {
    use super::E;
    use common::*;

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
