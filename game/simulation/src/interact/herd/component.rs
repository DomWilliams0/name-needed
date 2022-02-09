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
    herd: CurrentHerd,
}

#[derive(Debug, Copy, Clone)]
pub enum CurrentHerd {
    MemberOf(HerdHandle),
    PendingDeparture {
        herd: HerdHandle,
        ticks_remaining: u32,
    },
}

impl HerdedComponent {
    pub fn new(herd: HerdHandle) -> Self {
        Self {
            herd: CurrentHerd::MemberOf(herd),
        }
    }

    pub fn current(&self) -> CurrentHerd {
        self.herd
    }

    pub fn current_mut(&mut self) -> &mut CurrentHerd {
        &mut self.herd
    }
}

impl CurrentHerd {
    pub fn handle(self) -> HerdHandle {
        match self {
            CurrentHerd::MemberOf(herd) | CurrentHerd::PendingDeparture { herd, .. } => herd,
        }
    }
}

impl Display for CurrentHerd {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CurrentHerd::MemberOf(herd) => write!(f, "{:?}", herd),
            CurrentHerd::PendingDeparture {
                herd,
                ticks_remaining,
            } => write!(f, "{:?} (leaving in {})", herd, ticks_remaining),
        }
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
