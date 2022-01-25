pub use crate::definitions::ValueImpl;
use crate::ecs::*;
use crate::string::StringCache;
use common::Debug;
use std::any::Any;
use std::rc::Rc;

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

impl<V: Value> ComponentTemplateEntry<V> {
    pub fn new<T: ComponentTemplate<V>>(key: &'static str) -> Self {
        Self {
            key,
            construct_fn: T::construct,
        }
    }
}

#[macro_export]
macro_rules! register_component_template {
    ($key:expr, $ty:ident) => {
        inventory::submit!(ComponentTemplateEntry::new::<$ty>($key));
    };
}
