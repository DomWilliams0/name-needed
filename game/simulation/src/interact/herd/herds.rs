use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;

use crate::species::Species;
use crate::Entity;
use common::FmtResult;
use unit::world::{WorldPoint, WorldPointRange};

/// Resource to track herds
pub struct Herds {
    next: NonZeroU32,
    /// Holds active herds only
    herds: HashMap<HerdHandle, HerdInfo>,
}

#[derive(Clone, Debug)]
pub struct HerdInfo {
    pub median_pos: WorldPoint,
    pub range: WorldPointRange,
    pub members: usize,
    pub leader: Entity,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct HerdHandle {
    id: NonZeroU32,
    species: Species,
}

impl Herds {
    pub fn new_herd(&mut self, species: Species) -> HerdHandle {
        let herd = HerdHandle {
            id: self.next,
            species,
        };
        self.next = NonZeroU32::new(self.next.get() + 1).expect("herd handle overflow");

        herd
    }

    pub fn register_assigned_herds(&mut self, herds: impl Iterator<Item = (HerdHandle, HerdInfo)>) {
        self.herds.clear();
        self.herds.extend(herds);
    }

    pub fn get_info(&self, herd: HerdHandle) -> Option<&HerdInfo> {
        self.herds.get(&herd)
    }
}

impl Default for Herds {
    fn default() -> Self {
        Self {
            next: unsafe { NonZeroU32::new_unchecked(1) },
            herds: Default::default(),
        }
    }
}

impl Debug for HerdHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Herd({}, {})", self.id, self.species)
    }
}
