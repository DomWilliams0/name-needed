use crate::ecs::*;
use crate::simulation::Tick;
use crate::{PhysicalComponent, TransformComponent};
use common::*;
use std::borrow::Cow;
use unit::world::WorldPoint;

// TODO reimplement with octree

/// Implements efficient spatial entity queries
pub struct Spatial {
    /// Really stupid implementation for now
    entities: Vec<(Entity, WorldPoint)>,
}

/// Update spatial resource
pub struct SpatialSystem;

pub enum Transforms<'a> {
    Storage(&'a ReadStorage<'a, TransformComponent>),
    World(&'a EcsWorld),
}

impl Default for Spatial {
    fn default() -> Self {
        Self {
            entities: Vec::with_capacity(256),
        }
    }
}

impl Spatial {
    fn update(
        &mut self,
        entities: Read<EntitiesRes>,
        transforms: ReadStorage<TransformComponent>,
        physicals: ReadStorage<PhysicalComponent>,
    ) {
        self.entities.clear();

        for (e, transform, _) in (&entities, &transforms, &physicals).join() {
            self.entities.push((e.into(), transform.position));
        }

        if !self.entities.is_empty() {
            debug!(
                "updated spatial index with {count} entities",
                count = self.entities.len()
            );
        }
    }

    // The sort is a massive hotspot in profiling, keeping this not-inlined helps this terrible
    // TEMPORARY method stand out
    #[inline(never)]
    pub fn query_in_radius(
        &self,
        transforms: Transforms,
        centre: WorldPoint,
        radius: f32,
    ) -> impl Iterator<Item = (Entity, WorldPoint, f32)> {
        let transforms = match transforms {
            Transforms::Storage(storage) => Cow::Borrowed(storage),
            Transforms::World(w) => Cow::Owned(w.read_storage()),
        };

        let radius2 = radius.powi(2);

        // awful allocation in sort only acceptable because this is an awful temporary brute force
        // implementation
        self.entities
            .iter()
            .map(|(e, point)| {
                let distance2 = point.distance2(centre);
                (e, point, distance2)
            })
            // positions are cached so check transform is still present after filtering by distance
            .filter(|(e, _, dist2)| *dist2 < radius2 && transforms.contains((**e).into()))
            .map(|(e, point, dist2)| (*e, *point, dist2.sqrt()))
            .sorted_by_key(|(_, _, dist)| OrderedFloat(*dist))
    }
}

impl<'a> System<'a> for SpatialSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, PhysicalComponent>,
        Write<'a, Spatial>,
    );

    fn run(&mut self, (entities, transforms, physicals, mut spatial): Self::SystemData) {
        // only update occasionally
        let tick = Tick::fetch();
        if tick.value() % 8 == 0 {
            spatial.update(entities, transforms, physicals);
        }
    }
}

impl<'a> From<&'a ReadStorage<'a, TransformComponent>> for Transforms<'a> {
    fn from(t: &'a ReadStorage<'a, TransformComponent>) -> Self {
        Self::Storage(t)
    }
}

impl<'a> From<&'a EcsWorld> for Transforms<'a> {
    fn from(w: &'a EcsWorld) -> Self {
        Self::World(w)
    }
}
