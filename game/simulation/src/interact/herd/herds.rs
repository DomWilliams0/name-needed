use specs::WriteStorage;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;

use common::{trace, FmtResult};
use unit::world::{WorldPoint, WorldPointRange};

use crate::interact::herd::system::DiscoveredHerds;
use crate::species::Species;
use crate::{ComponentWorld, EcsWorld, Entity, HerdedComponent};

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

    /// Does not write to herded_comps, but the system has a mutable reference already
    pub(in crate::interact::herd) fn register_assigned_herds(
        &mut self,
        herded_comps: &WriteStorage<HerdedComponent>,
        herds: &mut DiscoveredHerds,
    ) {
        // don't bother reusing alloc, this happens only once and not very often
        let mut old_leaders = self
            .herds
            .drain()
            .map(|(h, info)| (herds.map_herd(h), info.leader))
            .collect::<HashMap<_, _>>();

        for (herd, herd_wip) in herds.iter_herds() {
            trace!(
                "registering herd {:?} with {} members",
                herd,
                herd_wip.count()
            );

            // find old leader, if any
            let leader = old_leaders.remove(&herd).and_then(|prev| {
                match prev.get(herded_comps) {
                    Some(comp) if comp.current().handle() == herd => Some(prev),
                    _ => None, // dead or not in the same herd anymore
                }
            });

            // find geometric median and possibly new leader
            let (leader, median) = match leader {
                None => {
                    let (leader, median) = herd_wip.choose_leader();
                    trace!("old leader is invalid, chose new"; "leader" => leader);
                    (leader, median)
                }
                Some(e) => {
                    trace!("keeping same leader"; "leader" => e);
                    (e, herd_wip.find_geometric_median())
                }
            };

            // register herd and leader
            let (min_pos, max_pos) = herd_wip.range();
            let range = WorldPointRange::with_inclusive_range(min_pos, max_pos);

            let herd_info = HerdInfo::new(median, leader, range, herd_wip.count());
            trace!("completed herd: {:?}", herd_info; "herd" => ?herd);
            self.herds.insert(herd, herd_info);
        }

        // TODO there might be old herd leaders to demote
        // TODO introduce promote and demote events again if needed
        debug_assert!(old_leaders.is_empty(), "old leaders: {:?}", old_leaders);
    }

    pub fn get_info(&self, herd: HerdHandle) -> Option<&HerdInfo> {
        self.herds.get(&herd)
    }

    pub fn iter(&self) -> impl Iterator<Item = (HerdHandle, &HerdInfo)> + '_ {
        self.herds.iter().map(|(h, info)| (*h, info))
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
