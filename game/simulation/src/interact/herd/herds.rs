use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;

use crate::species::Species;
use common::FmtResult;

/// Resource to track herds
pub struct Herds {
    next: NonZeroU32,
    alive: HashSet<HerdHandle>,
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

    pub fn register_assigned_herds(&mut self, herds: impl Iterator<Item = HerdHandle>) {
        self.alive.clear();
        self.alive.extend(herds);
    }
}

impl Default for Herds {
    fn default() -> Self {
        Self {
            next: unsafe { NonZeroU32::new_unchecked(1) },
            alive: Default::default(),
        }
    }
}

impl Debug for HerdHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Herd({}, {})", self.id, self.species)
    }
}
