use std::rc::Rc;

use common::*;

use crate::ecs::*;
use crate::interact::herd::HerdHandle;
use crate::StringCache;

/// Declares if an entity organises itself into herds with other members of the same species
#[derive(Component, EcsComponent, Clone, Debug, Default)]
#[storage(NullStorage)]
#[name("herdable")]
pub struct HerdableComponent;

/// An entity is part of a herd
#[derive(Component, EcsComponent, Debug)]
#[storage(HashMapStorage)]
#[name("herded")]
#[clone(disallow)]
pub struct HerdedComponent {
    herd: HerdHandle,
}

impl HerdedComponent {
    pub fn new(herd: HerdHandle) -> Self {
        Self { herd }
    }

    pub fn handle(&self) -> HerdHandle {
        self.herd
    }
}

impl<V: Value> ComponentTemplate<V> for HerdableComponent {
    fn construct(
        values: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        if !values.is_empty() {
            Err(ComponentBuildError::EmptyExpected)
        } else {
            Ok(Rc::new(Self))
        }
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(HerdableComponent)
    }

    crate::as_any!();
}

register_component_template!("herdable", HerdableComponent);
