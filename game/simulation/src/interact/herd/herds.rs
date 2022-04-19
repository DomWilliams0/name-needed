use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;

use common::FmtResult;
use unit::world::{WorldPoint, WorldPointRange};

use crate::species::Species;
use crate::Entity;

/// Resource to track herds
pub struct Herds {
    next: NonZeroU32,
    /// Holds active herds only
    herds: HashMap<HerdHandle, HerdInfo>,
}

#[derive(Clone, Debug)]
pub struct HerdInfo {
    /// Centre of herd, close to leader and used as fallback if leader is invalidated
    median_pos: WorldPoint,
    leader: Entity,
    range: WorldPointRange,
    members: usize,
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

impl HerdInfo {
    pub(in crate::interact::herd) fn new(
        median_pos: WorldPoint,
        leader: Entity,
        range: WorldPointRange,
        members: usize,
    ) -> Self {
        HerdInfo {
            median_pos,
            leader,
            range,
            members,
        }
    }

    pub const fn median_pos(&self) -> WorldPoint {
        self.median_pos
    }

    pub const fn range(&self) -> &WorldPointRange {
        &self.range
    }

    pub const fn member_count(&self) -> usize {
        self.members
    }

    /// Not guaranteed to be valid/alive
    pub fn leader_entity(&self) -> Entity {
        self.leader
    }

    /// Leader position or median herd pos if invalidated
    pub fn herd_centre(&self, get_pos: impl FnOnce(Entity) -> Option<WorldPoint>) -> WorldPoint {
        get_pos(self.leader).unwrap_or(self.median_pos)
    }
}
