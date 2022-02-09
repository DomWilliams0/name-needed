use std::collections::{HashSet, VecDeque};

use common::rstar::{Envelope, Point, RTree, AABB};
use common::*;
use unit::world::WorldPoint;

use crate::ecs::*;
use crate::interact::herd::component::{HerdableComponent, HerdedComponent};
use crate::interact::herd::herds::Herds;
use crate::interact::herd::HerdHandle;
use crate::simulation::EcsWorldRef;
use crate::spatial::Spatial;
use crate::{SpeciesComponent, Tick, TransformComponent};

/// Organises compatible entities into herds when nearby
pub struct HerdJoiningSystem;

#[derive(Clone, Copy, Debug)] // TODO remove debug
struct HerdTreeNode {
    pos: WorldPoint,
    entity: Entity,
    current_herd: Option<HerdHandle>,
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
        for (me, transform, _herdable, herd, _my_species) in (
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

            while let Some(top) = frontier.pop_front() {
                for nearby in tree.drain_within_distance(top.pos.xyz(), radius2) {
                    frontier.push_back(nearby);
                    current_herd.push(nearby);
                }
            }
            if current_herd.len() == 1 {
                // one man wolf pack, not a herd
                let _ = herded.remove(e.entity.into());
                continue;
            }

            // find an existing herd to reuse, or make a new one
            let herd = match current_herd.iter().find_map(|e| e.current_herd) {
                Some(herd) if !assigned_herds.contains(&herd) => herd,
                _ => herds.new_herd(),
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
