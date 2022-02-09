use std::collections::{HashSet, VecDeque};

use common::rstar::{Envelope, Point, PointDistance, RTree, AABB};
use common::*;
use unit::world::WorldPoint;

use crate::ecs::*;
use crate::interact::herd::component::{HerdableComponent, HerdedComponent};
use crate::interact::herd::herds::Herds;
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
        if Tick::fetch().value() % 12 != 0 {
            return;
        }

        // collect positions
        let mut entries = vec![];

        let radius2 = config::get().simulation.herd_radius.powi(2);
        for (me, transform, _herdable, herd, my_species) in (
            &entities,
            &transform,
            &herdable,
            (&herded).maybe(),
            &species,
        )
            .join()
        {
            entries.push(HerdTreeNode {
                entity: me.into(),
                pos: transform.position,
                current_herd: herd.map(|comp| comp.handle()),
                species: my_species.species(),
            })
        }

        let mut tree = RTree::bulk_load(entries);
        let mut assigned_herds = HashSet::new();

        while tree.size() != 0 {
            // find next herd
            // TODO reuse allocs
            let mut current_herd = vec![];
            let mut frontier = VecDeque::new();

            let e = *tree.iter().next().unwrap(); // checked to be not empty
            frontier.push_back(e);

            let species = e.species;

            while let Some(top) = frontier.pop_front() {
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
            if current_herd.len() == 1 {
                // one man wolf pack is not a herd
                let _ = herded.remove(e.entity.into());
                continue;
            }

            // find an existing herd to reuse, or make a new one
            let herd = match current_herd.iter().find_map(|e| e.current_herd) {
                Some(herd) if !assigned_herds.contains(&herd) => herd,
                _ => herds.new_herd(species),
            };

            // assign herd
            for e in current_herd.drain(..) {
                let _ = herded.insert(e.entity.into(), HerdedComponent::new(herd));
            }

            assigned_herds.insert(herd);
        }

        herds.register_assigned_herds(assigned_herds.iter().copied());
    }
}

mod rtree {
    use super::*;

    #[derive(Clone, Copy)]
    pub struct HerdTreeNode {
        pub pos: WorldPoint,
        pub entity: Entity,
        pub current_herd: Option<HerdHandle>,
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
