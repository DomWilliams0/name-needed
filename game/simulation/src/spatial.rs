use std::cell::RefCell;
use std::collections::HashSet;

use common::*;
use unit::world::WorldPoint;

use crate::ecs::*;
use crate::{PhysicalComponent, Tick, TransformComponent};

// TODO reimplement with octree

/// Implements efficient spatial entity queries
pub struct Spatial {
    inner: RefCell<SpatialInner>,
}

struct SpatialInner {
    /// Really stupid implementation for now
    positions: Vec<(Entity, WorldPoint)>,
    entities: HashSet<Entity>,
    newly_created_entities: Vec<Entity>,
}

/// Update spatial resource
pub struct SpatialSystem;

impl Default for Spatial {
    fn default() -> Self {
        Self {
            inner: RefCell::new(SpatialInner {
                positions: Vec::with_capacity(256),
                entities: HashSet::with_capacity(256),
                newly_created_entities: Vec::new(),
            }),
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
        let mut inner = self.inner.borrow_mut();

        inner.positions.clear();
        inner.entities.clear();

        for (e, transform, _) in (&entities, &transforms, &physicals).join() {
            inner.positions.push((e.into(), transform.position));
            inner.entities.insert(e.into());
        }

        // already included new entities
        debug_assert!(inner
            .newly_created_entities
            .iter()
            .all(|e| inner.positions.iter().any(|(existing, _)| existing == e)));
        inner.newly_created_entities.clear();

        if !inner.positions.is_empty() {
            trace!(
                "updated spatial index with {count} entities",
                count = inner.positions.len()
            );
        }
    }

    /// Will appear in queries immediately, even before system runs
    pub fn register_new_entity(&self, entity: Entity) {
        self.inner.borrow_mut().newly_created_entities.push(entity);
    }

    // The sort is a massive hotspot in profiling, keeping this not-inlined helps this terrible
    // TEMPORARY method stand out
    #[inline(never)]
    pub fn query_in_radius(
        &self,
        world: &EcsWorld,
        centre: WorldPoint,
        radius: f32,
    ) -> impl Iterator<Item = (Entity, WorldPoint, f32)> {
        let mut inner = self.inner.borrow_mut();
        let transforms = world.read_storage::<TransformComponent>();

        // add any new entities
        if !inner.newly_created_entities.is_empty() {
            let physicals = world.read_storage::<PhysicalComponent>();

            let max = inner.newly_created_entities.len();
            let mut total = 0;
            let mut new_entities = std::mem::take(&mut inner.newly_created_entities);
            for new_entity in new_entities.drain(..) {
                if let Some((transform, _)) =
                    world.components(new_entity, (&transforms, &physicals))
                {
                    if inner.entities.insert(new_entity) {
                        inner.positions.push((new_entity, transform.position));
                        total += 1;
                    }
                }
            }

            // put vec back
            std::mem::forget(std::mem::replace(
                &mut inner.newly_created_entities,
                new_entities,
            ));

            trace!(
                "updated spatial with {}/{} of newly created entities",
                total,
                max
            );
        }

        let radius2 = radius.powi(2);

        // awful allocation in sort only acceptable because this is an awful temporary brute force
        // implementation
        inner
            .positions
            .iter()
            .map(|(e, point)| {
                let distance2 = point.distance2(centre);
                (e, point, distance2)
            })
            // positions are cached so check transform is still present after filtering by distance
            .filter(|(e, _, dist2)| *dist2 < radius2 && e.has(&transforms))
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
        if tick.value() % 5 == 0 {
            spatial.update(entities, transforms, physicals);
        }
    }
}
