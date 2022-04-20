use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;

use common::FmtResult;
use unit::world::{WorldPoint, WorldPointRange};

use crate::event::EntityEventQueue;
use crate::species::Species;
use crate::{ComponentWorld, EcsWorld, Entity, EntityEvent, EntityEventPayload, HerdedComponent};

type HerdId = NonZeroU32;

/// Resource to track herds
pub struct Herds {
    next: HerdId,
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

/// Unstable and ephemeral, should not be stored
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct HerdHandle {
    id: HerdId,
    // TODO map species to a separate id count? or just use a u64
    species: Species,
}

impl Herds {
    pub fn new_herd(&mut self, species: Species) -> HerdHandle {
        let herd = HerdHandle {
            id: self.next,
            species,
        };
        self.next = HerdId::new(self.next.get() + 1).expect("herd handle overflow");

        herd
    }

    pub fn register_assigned_herds(
        &mut self,
        world: &EcsWorld,
        herds: impl Iterator<Item = (HerdHandle, HerdInfo)>,
    ) {
        // TODO reuse allocs
        let mut old_leaders = self
            .herds
            .drain()
            .map(|(h, info)| (info.leader, h))
            .collect::<HashSet<_>>();
        let mut new_leaders = vec![];

        for (herd, info) in herds {
            if old_leaders.remove(&(info.leader, herd)) {
                // no change, this entity was already the leader for this herd
            } else {
                // demote old leader and promote new, deferring promotion event until after demotion
                new_leaders.push(info.leader);
            }

            self.herds.insert(herd, info);
        }

        let events = world.resource_mut::<EntityEventQueue>();

        // demote old leaders first
        events.post_multiple(old_leaders.drain().map(|(e, herd)| EntityEvent {
            subject: e,
            payload: EntityEventPayload::DemotedFromHerdLeader(herd),
        }));

        // promote new leaders
        events.post_multiple(new_leaders.drain(..).map(|e| EntityEvent {
            subject: e,
            payload: EntityEventPayload::PromotedToHerdLeader,
        }));
    }

    pub fn get_info(&self, herd: HerdHandle) -> Option<&HerdInfo> {
        self.herds.get(&herd)
    }
}

impl Default for Herds {
    fn default() -> Self {
        Self {
            next: unsafe { HerdId::new_unchecked(1) },
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

    /// Helper
    pub fn get(entity: Entity, world: &EcsWorld) -> Option<&HerdInfo> {
        world
            .component::<HerdedComponent>(entity)
            .ok()
            .and_then(|comp| {
                let herds = world.resource::<Herds>();
                herds.get_info(comp.current().handle())
            })
    }
}
