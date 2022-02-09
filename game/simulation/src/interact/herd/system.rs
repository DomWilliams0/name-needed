use std::collections::{HashMap, VecDeque};

use common::rstar::{Envelope, Point, PointDistance, RTree, AABB};
use common::*;
use unit::world::{WorldPoint, WorldPointRange};

use crate::ecs::*;
use crate::interact::herd::component::{CurrentHerd, HerdableComponent, HerdedComponent};
use crate::interact::herd::herds::{HerdInfo, Herds};
use crate::interact::herd::system::rtree::{HerdTreeNode, SpeciesSelectionFunction};
use crate::interact::herd::HerdHandle;
use crate::simulation::EcsWorldRef;
use crate::spatial::Spatial;
use crate::species::Species;
use crate::{SpeciesComponent, Tick, TransformComponent};

/// Organises compatible entities into herds when nearby
pub struct HerdJoiningSystem;

impl<'a> System<'a> for HerdJoiningSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldRef>,
        Read<'a, Spatial>,
        Write<'a, Herds>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, HerdableComponent>,
        WriteStorage<'a, HerdedComponent>,
        ReadStorage<'a, SpeciesComponent>,
    );

    fn run(
        &mut self,
        (entities, _world, _spatial, mut herds, transform, herdable, mut herded, species): Self::SystemData,
    ) {
        // run occasionally
        const RUN_FREQUENCY: u32 = 5;
        if Tick::fetch().value() % RUN_FREQUENCY != 0 {
            return;
        }

        let (radius2, ticks_until_departure) = {
            let cfg = &config::get().simulation;
            (cfg.herd_radius.powi(2), cfg.herd_expiry_ticks)
        };

        // collect positions
        let mut entries = vec![];

        for (me, transform, _herdable, herd, my_species) in (
            &entities,
            &transform,
            &herdable,
            (&mut herded).maybe(),
            &species,
        )
            .join()
        {
            let current_herd = herd.map(|comp| {
                if let CurrentHerd::PendingDeparture {
                    ticks_remaining, ..
                } = comp.current_mut()
                {
                    *ticks_remaining = ticks_remaining.saturating_sub(RUN_FREQUENCY);
                }
                comp.current()
            });
            entries.push(HerdTreeNode {
                entity: me.into(),
                pos: transform.position,
                current_herd,
                species: my_species.species(),
            })
        }

        let mut tree = RTree::bulk_load(entries);
        let mut discovered = DiscoveredHerds::default();

        // TODO reuse allocs
        let mut current_herd = Vec::new();
        let mut frontier = VecDeque::new();
        while tree.size() != 0 {
            debug_assert!(current_herd.is_empty());

            // find next herd
            let e = *tree.iter().next().unwrap(); // checked to be not empty
            frontier.push_back(e);

            let species = e.species;
            while let Some(top) = frontier.pop_front() {
                trace!("considering top of frontier"; top.entity);

                if let Some(CurrentHerd::PendingDeparture {
                    herd,
                    ticks_remaining,
                }) = top.current_herd
                {
                    if ticks_remaining > 0 {
                        trace!("{}: keeping in old herd during departure", top.entity; "herd" => ?herd, "ticks" => ?ticks_remaining);
                        // keep in old herd still
                        discovered.add_member(herd, top);

                        // remove from tree if not already drained
                        tree.remove_at_point(&top.pos.xyz());

                        // do not extend the current herd with this one
                        continue;
                    }
                    trace!("{}: time to remove from expired", top.entity; "herd" => ?herd);
                }

                let selection = SpeciesSelectionFunction {
                    circle_origin: top.pos.xyz(),
                    squared_max_distance: radius2,
                    species,
                };

                for nearby in tree.drain_with_selection_function(selection) {
                    frontier.push_back(nearby);
                    current_herd.push(nearby);
                }
            }

            if current_herd.len() == 0 {
                trace!("empty herd");
                continue;
            } else if current_herd.len() == 1 {
                // one man wolf pack is not a herd, leave current
                let leaver = current_herd.pop().unwrap(); // length was checked

                match leaver.current_herd {
                    Some(CurrentHerd::MemberOf(herd)) => {
                        // remain a member of old herd
                        trace!("{}: has wandered away from herd, start departure", leaver.entity; "herd" => ?herd);
                        discovered.add_member(herd, leaver);

                        // start leaving
                        let herd_to_leave = leaver
                            .entity
                            .get_mut(&mut herded)
                            .expect("should be in a herd already");

                        *herd_to_leave.current_mut() = CurrentHerd::PendingDeparture {
                            herd,
                            ticks_remaining: ticks_until_departure,
                        };
                    }
                    Some(CurrentHerd::PendingDeparture {
                        ticks_remaining,
                        herd,
                    }) => {
                        debug_assert_eq!(ticks_remaining, 0);
                        let _ = herded.remove(e.entity.into());
                        trace!("{}: has finally departed", leaver.entity; "herd" => ?herd);
                    }
                    None => {}
                }

                continue;
            }

            trace!("time to create a herd from {} members", current_herd.len());

            // find an existing herd to reuse, or make a new one
            let herd = {
                let mut winner = None;
                current_herd
                    .iter()
                    .filter_map(|e| e.current_herd)
                    .for_each(|herd| {
                        let herd = herd.handle();
                        match winner {
                            None => {
                                winner = Some(herd);
                                trace!("winning herd {:?} chosen", herd);
                            }
                            Some(winner) => {
                                if winner != herd {
                                    discovered.register_mapping(herd, winner);
                                    trace!(
                                        "losing herd {:?} replaced by winner {:?}",
                                        herd,
                                        winner
                                    );
                                }
                            }
                        }
                    });

                winner.unwrap_or_else(|| {
                    let herd = herds.new_herd(species);
                    trace!("allocating new herd {:?}", herd);
                    herd
                })
            };

            for e in current_herd.drain(..) {
                discovered.add_member(herd, e);

                // unconditionally add to this current herd
                let _ = herded.insert(e.entity.into(), HerdedComponent::new(herd));
            }
        }

        // update pending departures from mapping
        for (herded,) in (&mut herded,).join() {
            match herded.current_mut() {
                CurrentHerd::PendingDeparture { herd, .. } | CurrentHerd::MemberOf(herd) => {
                    if let Some(new) = discovered.mapping.get(herd) {
                        trace!("remapping from {:?} to {:?}", *herd, *new);
                        *herd = *new;
                    }
                }
            }
        }

        // register alive herds
        herds.register_assigned_herds(discovered.finish());
    }
}

struct HerdInProgress {
    pub summed_pos: (f32, f32, f32),
    pub min_pos: (f32, f32, f32),
    pub max_pos: (f32, f32, f32),
    pub members: usize,
}

#[derive(Default)]
struct DiscoveredHerds {
    herds: HashMap<HerdHandle, HerdInProgress>,
    mapping: HashMap<HerdHandle, HerdHandle>,
}

impl DiscoveredHerds {
    fn add_member(&mut self, herd: HerdHandle, member: HerdTreeNode) {
        let e = {
            let key = self.mapping.get(&herd).copied().unwrap_or(herd);
            self.herds.entry(key).or_insert(HerdInProgress::default())
        };

        let (x, y, z) = member.pos.xyz();
        e.summed_pos = (e.summed_pos.0 + x, e.summed_pos.1 + y, e.summed_pos.2 + z);
        e.min_pos = (e.min_pos.0.min(x), e.min_pos.1.min(y), e.min_pos.2.min(z));
        e.max_pos = (e.max_pos.0.max(x), e.max_pos.1.max(y), e.max_pos.2.max(z));
        e.members += 1;
    }

    fn register_mapping(&mut self, old: HerdHandle, new: HerdHandle) {
        if let Some(prev) = self.mapping.insert(old, new) {
            if prev != new {
                warn!(
                    "overwrote herd mapping {:?}:{:?} with {:?}:{:?}",
                    old, prev, old, new
                );
            }
        }
    }

    fn finish(&mut self) -> impl Iterator<Item = (HerdHandle, HerdInfo)> + '_ {
        self.herds.drain().map(|(herd, wip)| {
            let average_pos = {
                debug_assert_ne!(wip.members, 0);
                let n = wip.members as f32;
                WorldPoint::new(
                    wip.summed_pos.0 / n,
                    wip.summed_pos.1 / n,
                    wip.summed_pos.2 / n,
                )
                .expect("invalid herd average position")
            };
            let range = {
                let from = WorldPoint::new(wip.min_pos.0, wip.min_pos.1, wip.min_pos.2);
                let to = WorldPoint::new(wip.max_pos.0, wip.max_pos.1, wip.max_pos.2);

                let (from, to) = from.zip(to).expect("invalid herd min/max position");
                WorldPointRange::with_inclusive_range(from, to)
            };

            let out = (
                herd,
                HerdInfo {
                    average_pos,
                    range,
                    members: wip.members,
                },
            );
            trace!("completed herd: {:?}", out);
            out
        })
    }
}

impl Default for HerdInProgress {
    fn default() -> Self {
        HerdInProgress {
            summed_pos: (0.0, 0.0, 0.0),
            min_pos: (f32::MAX, f32::MAX, f32::MAX),
            max_pos: (f32::MIN, f32::MIN, f32::MIN),
            members: 0,
        }
    }
}

mod rtree {
    use crate::interact::herd::component::CurrentHerd;

    use super::*;

    #[derive(Clone, Copy, Debug)]
    pub struct HerdTreeNode {
        pub pos: WorldPoint,
        pub entity: Entity,
        pub current_herd: Option<CurrentHerd>,
        pub species: Species,
    }

    /// Allows for filtering drained nodes to those matching the species
    pub struct SpeciesSelectionFunction {
        pub circle_origin: (f32, f32, f32),
        pub squared_max_distance: f32,
        pub species: Species,
    }

    impl rstar::RTreeObject for HerdTreeNode {
        type Envelope = AABB<(f32, f32, f32)>;

        fn envelope(&self) -> Self::Envelope {
            AABB::from_point(self.pos.xyz())
        }
    }

    impl rstar::PointDistance for HerdTreeNode {
        fn distance_2(
            &self,
            other: &<Self::Envelope as Envelope>::Point,
        ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
            Point3::from(self.pos).distance2(Point3::from(*other))
        }
    }

    impl rstar::SelectionFunction<HerdTreeNode> for SpeciesSelectionFunction {
        fn should_unpack_parent(&self, parent_envelope: &AABB<(f32, f32, f32)>) -> bool {
            let envelope_distance = parent_envelope.distance_2(&self.circle_origin);
            envelope_distance <= self.squared_max_distance
        }

        fn should_unpack_leaf(&self, leaf: &HerdTreeNode) -> bool {
            leaf.species == self.species
                && leaf
                    .distance_2_if_less_or_equal(&self.circle_origin, self.squared_max_distance)
                    .is_some()
        }
    }
}
