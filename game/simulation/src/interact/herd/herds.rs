use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;

use common::FmtResult;

/// Resource to track herds
pub struct Herds {
    next: NonZeroU32,
    alive: HashSet<HerdHandle>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct HerdHandle(NonZeroU32);

impl Herds {
    pub fn new_herd(&mut self) -> HerdHandle {
        let herd = HerdHandle(self.next);
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
        write!(f, "Herd({})", self.0)
    }
}
