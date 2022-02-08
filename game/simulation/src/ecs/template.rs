use std::any::Any;
use std::rc::Rc;

use common::Debug;

pub use crate::definitions::ValueImpl;
use crate::ecs::*;
use crate::string::StringCache;

pub trait ComponentTemplate<V: Value>: Debug {
    fn construct(
        values: &mut Map<V>,
        string_cache: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized;

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b>;

    fn as_any(&self) -> &dyn Any;
}

#[derive(Clone)]
pub struct ComponentTemplateEntry<V: Value> {
    pub key: &'static str,
    pub construct_fn:
        fn(&mut Map<V>, &StringCache) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>,
}

inventory::collect!(ComponentTemplateEntry<ValueImpl>);

#[macro_export]
macro_rules! register_component_template {
    ($key:expr, $ty:ident) => {
        inventory::submit!(ComponentTemplateEntry {
            key: $key,
            construct_fn: $ty::construct
        });
    };
}
