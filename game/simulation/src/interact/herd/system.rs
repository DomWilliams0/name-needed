use std::collections::{HashMap, VecDeque};

use daggy::petgraph::graph::DiGraph;

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

const RUN_FREQUENCY: u32 = 5;

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
        if Tick::fetch().value() % RUN_FREQUENCY != 0 {
            return;
        }

        let ticks_until_departure = config::get().simulation.herd_expiry_ticks;

        // query tree to create graph of connected herdable entities
        // TODO reuse allocs
        let rtree = init_rtree(&entities, &transform, &herdable, &herded, &species);
        let connectivity = discover_connectivity(rtree);

        let mut subgraphs = collect_subgraphs(connectivity);

        // put single entities at the end, to be processed after we've seen all the herds and their mappings
        subgraphs.sort_unstable_by_key(|subgraph| matches!(subgraph, Subgraph::Single(_)));
        trace!("subgraphs: {:?}", subgraphs);

        let mut discovered_herds = DiscoveredHerds::default();
        let mut herd_member_count = HashMap::new();

        for subgraph in subgraphs {
            trace!("processing subgraph: {:?}", subgraph);

            match subgraph {
                // all manys come before singles
                Subgraph::Many(members) => {
                    debug_assert!(!members.is_empty());

                    // count members to find dominant herd
                    herd_member_count.clear(); // from last iteration
                    for member in members.iter() {
                        if let Some(current) = member
                            .entity
                            .get(&herded)
                            .map(|comp| comp.current().handle())
                        {
                            *herd_member_count.entry(current).or_insert(0) += 1;
                        }
                    }

                    trace!("herd counts: {:?}", herd_member_count);

                    let winning_herd = herd_member_count
                        .iter()
                        .filter(|(herd, _)| !discovered_herds.herds.contains_key(*herd))
                        .max_by_key(|(_, count)| **count)
                        .map(|(herd, _)| *herd)
                        .unwrap_or_else(|| {
                            let member = members.first().unwrap(); // not empty
                            let species = member.entity.get(&species).expect("missing species");
                            trace!("allocating new herd");
                            herds.new_herd(species.species())
                        });

                    trace!("winning herd is {:?}", winning_herd);

                    // assign to new herd
                    for member in members.iter() {
                        let prev = herded
                            .insert(member.entity.into(), HerdedComponent::new(winning_herd))
                            .ok()
                            .flatten()
                            .map(|comp| comp.current().handle());

                        if let Some(prev) = prev {
                            if prev != winning_herd
                                && discovered_herds.register_mapping(prev, winning_herd)
                            {
                                trace!(
                                    "losing herd {:?} replaced by winner {:?}",
                                    prev,
                                    winning_herd
                                );
                            }
                        }
                        discovered_herds.add_member(winning_herd, member.pos);
                    }
                }
                Subgraph::Single(alone) => {
                    let herd = match alone.entity.get_mut(&mut herded) {
                        Some(comp) => comp,
                        None => {
                            // doesn't have one, doesn't need one
                            trace!("already not in a herd");
                            continue;
                        }
                    };

                    let current = herd.current_mut();
                    let new_mapped_herd = discovered_herds.mapping.get(&current.handle()).copied();
                    match current {
                        CurrentHerd::MemberOf(herd) => {
                            let prev_herd = *herd;
                            let departing_herd = new_mapped_herd.unwrap_or(prev_herd);
                            *current = CurrentHerd::PendingDeparture {
                                herd: departing_herd,
                                ticks_remaining: ticks_until_departure,
                            };
                            trace!(
                                "now pending departure from {:?} (previously {:?})",
                                departing_herd,
                                prev_herd,
                            );
                        }
                        CurrentHerd::PendingDeparture {
                            herd,
                            ticks_remaining,
                        } => {
                            *ticks_remaining = ticks_remaining.saturating_sub(RUN_FREQUENCY);
                            if *ticks_remaining == 0 {
                                // finished
                                let left_herd = *herd;
                                let _ = herded.remove(alone.entity.into());
                                trace!("finally quitting herd {:?}", left_herd);
                                continue; // no longer part of herd
                            } else if let Some(new_herd) = new_mapped_herd {
                                trace!("while pending departure, remapped {:?} to {:?}", *herd, new_herd; "ticks" => *ticks_remaining);
                                *herd = new_herd;
                            }
                        }
                    }

                    // register as part of herd
                    discovered_herds.add_member(current.handle(), alone.pos);
                }
            }
        }

        // register alive herds
        herds.register_assigned_herds(discovered_herds.finish());
    }
}

#[derive(Copy, Clone, Derivative)]
#[derivative(Debug)]
struct HerdedEntity {
    entity: Entity,
    #[derivative(Debug = "ignore")]
    pos: WorldPoint,
}

struct ConnectivityNode {
    entity: HerdedEntity,
    visited: bool,
}

#[derive(Debug)]
enum Subgraph {
    Single(HerdedEntity),
    Many(Vec<HerdedEntity>),
}

fn init_rtree(
    entities: &Read<EntitiesRes>,
    transform: &ReadStorage<TransformComponent>,
    herdable: &ReadStorage<HerdableComponent>,
    herded: &WriteStorage<HerdedComponent>,
    species: &ReadStorage<SpeciesComponent>,
) -> RTree<HerdTreeNode> {
    let mut entries = vec![];
    for (me, transform, _herdable, herd, my_species) in
        (entities, transform, herdable, herded.maybe(), species).join()
    {
        let current_herd = herd.map(|comp| comp.current());
        entries.push(HerdTreeNode {
            entity: me.into(),
            pos: transform.position,
            current_herd,
            species: my_species.species(),
        })
    }

    RTree::bulk_load(entries)
}

fn discover_connectivity(mut rtree: RTree<HerdTreeNode>) -> DiGraph<ConnectivityNode, ()> {
    let mut connectivity = DiGraph::with_capacity(rtree.size(), rtree.size() / 2);
    let radius2 = config::get().simulation.herd_radius.powi(2);

    let mut frontier = VecDeque::new();
    let mut node_lookup = HashMap::new();

    while rtree.size() != 0 {
        // checked to be not empty
        let e = *rtree.iter().next().unwrap();

        trace!("next herd root"; e.entity);
        frontier.push_back(e);

        let species = e.species;
        while let Some(top) = frontier.pop_front() {
            trace!("considering top of frontier"; top.entity);

            let src_node = *node_lookup.entry(top.entity).or_insert_with(|| {
                connectivity.add_node(ConnectivityNode {
                    entity: HerdedEntity {
                        entity: top.entity,
                        pos: top.pos,
                    },
                    visited: false,
                })
            });

            let nearby_entities = rtree
                .drain_with_selection_function(SpeciesSelectionFunction {
                    circle_origin: top.pos.xyz(),
                    squared_max_distance: radius2,
                    species,
                })
                .filter(|nearby| nearby.entity != top.entity);

            for nearby in nearby_entities {
                trace!("adding nearby to frontier"; nearby.entity);
                frontier.push_back(nearby);

                let dst_node = *node_lookup.entry(nearby.entity).or_insert_with(|| {
                    connectivity.add_node(ConnectivityNode {
                        entity: HerdedEntity {
                            entity: nearby.entity,
                            pos: nearby.pos,
                        },
                        visited: false,
                    })
                });
                connectivity.add_edge(src_node, dst_node, ());
            }
        }
    }
    connectivity
}

fn collect_subgraphs(mut connectivity: DiGraph<ConnectivityNode, ()>) -> Vec<Subgraph> {
    let mut subgraphs = vec![];
    let mut this_herd = vec![];
    let mut frontier = VecDeque::new();

    for current_idx in connectivity.node_indices() {
        let current_node = connectivity.node_weight_mut(current_idx).unwrap();
        if current_node.visited {
            continue;
        }

        debug_assert!(frontier.is_empty());
        debug_assert!(this_herd.is_empty());
        frontier.push_back(current_idx);

        while let Some(top) = frontier.pop_front() {
            let top_node = connectivity.node_weight_mut(top).unwrap();
            debug_assert!(!top_node.visited);

            this_herd.push(top_node.entity);
            top_node.visited = true;

            for neighbour in connectivity.neighbors(top) {
                let neighbour_node = connectivity.node_weight(neighbour).unwrap();
                if !neighbour_node.visited {
                    frontier.push_back(neighbour);
                }
            }
        }

        let subgraph = if this_herd.len() == 1 {
            Subgraph::Single(this_herd.pop().unwrap())
        } else {
            Subgraph::Many(this_herd.drain(..).collect())
        };
        subgraphs.push(subgraph);
    }

    subgraphs
}

struct HerdInProgress {
    summed_pos: (f32, f32, f32),
    min_pos: (f32, f32, f32),
    max_pos: (f32, f32, f32),
    members: usize,
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

#[derive(Default)]
struct DiscoveredHerds {
    herds: HashMap<HerdHandle, HerdInProgress>,
    mapping: HashMap<HerdHandle, HerdHandle>,
}

impl DiscoveredHerds {
    fn add_member(&mut self, herd: HerdHandle, pos: WorldPoint) {
        let e = {
            let key = self.mapping.get(&herd).copied().unwrap_or(herd);
            self.herds
                .entry(key)
                .or_insert_with(HerdInProgress::default)
        };

        let (x, y, z) = pos.xyz();
        e.summed_pos = (e.summed_pos.0 + x, e.summed_pos.1 + y, e.summed_pos.2 + z);
        e.min_pos = (e.min_pos.0.min(x), e.min_pos.1.min(y), e.min_pos.2.min(z));
        e.max_pos = (e.max_pos.0.max(x), e.max_pos.1.max(y), e.max_pos.2.max(z));
        e.members += 1;
    }

    /// Returns true if not a duplicate
    fn register_mapping(&mut self, old: HerdHandle, new: HerdHandle) -> bool {
        if let Some(prev) = self.mapping.insert(old, new) {
            if prev != new {
                warn!(
                    "overwrote herd mapping {:?}:{:?} with {:?}:{:?}",
                    old, prev, old, new
                );
                return true;
            }
        }

        false
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
