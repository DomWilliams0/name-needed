use crate::ecs::*;
use crate::job::SocietyJobHandle;
use crate::string::{CachedStr, StringCache};
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU16;

#[derive(Hash, Clone, Eq, PartialEq)]
pub struct BuildMaterial {
    // TODO flexible list of reqs based on components
    definition_name: CachedStr,
    quantity: NonZeroU16,
}

/// Reserved for a build job
#[derive(Component, EcsComponent, Debug)]
#[storage(HashMapStorage)]
#[name("reserved-material")]
#[clone(disallow)]
pub struct ReservedMaterialComponent {
    pub build_job: SocietyJobHandle,
}

/// In the process of being consumed for a build job
#[derive(Component, EcsComponent, Debug, Default)]
#[storage(NullStorage)]
#[name("consumed-material")]
#[clone(disallow)]
pub struct ConsumedMaterialForJobComponent;

impl BuildMaterial {
    /// Cheap
    pub fn new(definition_name: CachedStr, quantity: NonZeroU16) -> Self {
        Self {
            definition_name,
            quantity,
        }
    }

    // TODO remove this
    #[deprecated]
    pub fn new_str(definition_name: &str, quantity: NonZeroU16) -> Self {
        Self {
            definition_name: StringCache::get_temporary(definition_name),
            quantity,
        }
    }

    pub fn definition(&self) -> CachedStr {
        self.definition_name
    }

    pub fn quantity(&self) -> NonZeroU16 {
        self.quantity
    }
}

impl Debug for BuildMaterial {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.quantity, self.definition_name.as_ref())
    }
}
