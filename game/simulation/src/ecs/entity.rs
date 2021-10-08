use common::*;
use std::ops::{Deref, DerefMut};

/// Wrapper around specs entity to extend it
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
// TODO custom hash? just itself
pub struct Entity(specs::Entity);

/// A copy of [Entity] but possible to create manually from index+generation, for scripting
/// purposes.
///
/// It's technically undefined to transmute like this but there's a unit test to confirm it's valid.
/// We might eventually reimplement the ECS ourselves too
#[derive(Copy, Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct EntityWrapper(pub specs::world::Index, pub std::num::NonZeroI32);

impl Display for EntityWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&Entity::from(*self), f)
    }
}

impl Deref for Entity {
    type Target = specs::Entity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Entity {
    pub fn has<C: specs::Component, D: Deref<Target = specs::storage::MaskedStorage<C>>>(
        self,
        storage: &specs::Storage<C, D>,
    ) -> bool {
        storage.contains(self.0)
    }

    pub fn get<'e, C: specs::Component, D: Deref<Target = specs::storage::MaskedStorage<C>>>(
        self,
        storage: &'e specs::Storage<C, D>,
    ) -> Option<&'e C> {
        storage.get(self.0)
    }

    pub fn get_mut<
        'e,
        C: specs::Component,
        D: DerefMut<Target = specs::storage::MaskedStorage<C>>,
    >(
        self,
        storage: &'e mut specs::Storage<C, D>,
    ) -> Option<&'e mut C> {
        storage.get_mut(self.0)
    }
}

impl Display for Entity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "E{}:{}", self.0.gen().id(), self.0.id())
    }
}

impl Debug for Entity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl From<specs::Entity> for Entity {
    #[inline(always)]
    fn from(e: specs::Entity) -> Self {
        Self(e)
    }
}

impl From<Entity> for specs::Entity {
    #[inline(always)]
    fn from(e: Entity) -> Self {
        e.0
    }
}

impl slog::KV for Entity {
    fn serialize(&self, _: &Record, serializer: &mut dyn Serializer) -> SlogResult<()> {
        serializer.emit_arguments("entity", &format_args!("{}", self))
    }
}

impl slog::Value for Entity {
    fn serialize(&self, _: &Record, key: Key, serializer: &mut dyn Serializer) -> SlogResult<()> {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

impl From<EntityWrapper> for Entity {
    fn from(e: EntityWrapper) -> Self {
        // safety: see doc comment on EntityWrapper (and unit test below)
        let specs = unsafe { std::mem::transmute::<_, specs::Entity>(e) };
        Self(specs)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroI32;

    use crate::ecs::{Builder, EntityWrapper, WorldExt};

    use super::*;

    #[test]
    fn entity_id_conversion() {
        let mut world = specs::World::new();

        for i in 0..50 {
            let e = world.create_entity().build();
            eprintln!("{:?}", e);

            let index = e.id();
            let gen = e.gen();

            let my_e = EntityWrapper(index, NonZeroI32::new(gen.id()).unwrap());
            let my_e = Entity::from(my_e);
            assert_eq!(e, my_e.0, "specs entity layout has changed");
            assert_eq!(Entity::from(e), my_e, "specs entity layout has changed");

            if i % 2 == 0 {
                // try out some other generations too
                world.delete_entity(e).unwrap();
            }
        }
    }
}
