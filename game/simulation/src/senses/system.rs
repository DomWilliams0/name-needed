use crate::ecs::*;
use crate::senses::sense::{HearingSphere, Sense, VisionCone};
use crate::spatial::Spatial;
use crate::TransformComponent;
use common::*;
use serde::Deserialize;
use specs::{Builder, EntityBuilder};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use unit::world::WorldPoint;

/// Ticks to remember a sensed entity
const SENSE_DECAY: u8 = 40;

/// Populated by other systems with the available sense ranges
#[derive(Component, EcsComponent, Default)]
#[storage(DenseVecStorage)]
#[name("senses")]
pub struct SensesComponent {
    pub vision: ArrayVec<[VisionCone; 1]>,
    pub hearing: ArrayVec<[HearingSphere; 1]>,

    /// Sensed entities this tick
    /// TODO maybe the ecs bitmask can be reused here instead of a huge alloc per entity
    sensed: Vec<SensedEntity>,
}

struct SensedEntity {
    entity: Entity,
    how: Sense,
    /// Expired when 0
    decay: u8,
}

/// Dummy magical sense provider - this will be replaced by individual body parts that provide the
/// senses
#[derive(Component, EcsComponent, Debug, Clone)]
#[name("magical-senses")]
#[storage(DenseVecStorage)]
pub struct MagicalSenseComponent {
    pub vision: VisionCone,
    // pub hearing: HearingSphere,
}

pub struct SensesSystem;

impl<'a> System<'a> for SensesSystem {
    type SystemData = (
        Read<'a, Spatial>,
        Read<'a, EntitiesRes>,
        ReadStorage<'a, MagicalSenseComponent>,
        ReadStorage<'a, TransformComponent>,
        WriteStorage<'a, SensesComponent>,
    );

    fn run(&mut self, (spatial, entities, providers, transforms, mut senses): Self::SystemData) {
        log_scope!(o!("system" => "senses"));

        // TODO system is expensive, dont run every tick
        // TODO consider using expiry times rather than decrementing a decay counter

        // update sense capabilities
        for (e, provider, senses) in (&entities, &providers, &mut senses).join() {
            let prev_hash = senses.debug_hash();
            senses.clear();

            // no calculation needed atm, just copy the sense definition directly into the senses
            senses.vision.push(provider.vision.clone());
            // senses.hearing.push(provider.hearing.clone());

            if senses.debug_hash() != prev_hash {
                debug!("senses updated"; E(e), "senses" => ?senses)
            }
        }

        // use senses
        for (e, senses, transform) in (&entities, &mut senses, &transforms).join() {
            log_scope!(o!(E(e)));

            senses.decay_sensed_entities();

            // do a single query for all senses
            let max_radius = match senses.max_radius() {
                Some(f) => f,
                None => {
                    // no senses
                    trace!("no senses");
                    continue;
                }
            };

            // TODO specialize query e.g. only detect those with a given component combo e.g. Transform + Render (+ Visible/!Invisible?)

            spatial
                .query_in_radius(transform.position, max_radius)
                .filter(|(entity, _, _)| *entity != e) // dont sense yourself
                .for_each(|(entity, pos, dist)| {
                    let sensed = senses.senses(transform, &pos, dist);
                    if !sensed.is_empty() {
                        senses.add_sensed_entity(entity, sensed);
                    }
                });

            trace!("senses {count} entities", count = senses.sensed.len());
        }
    }
}

impl SensesComponent {
    fn clear(&mut self) {
        self.vision.clear();
        self.hearing.clear();
    }

    fn max_radius(&self) -> Option<f32> {
        let vision = self.vision.iter().map(|v| v.length);
        let hearing = self.hearing.iter().map(|h| h.radius);

        vision.chain(hearing).max_by_key(|f| OrderedFloat(*f))
    }

    fn debug_hash(&self) -> u64 {
        if logger().is_debug_enabled() {
            let mut hasher = DefaultHasher::new();
            self.vision.hash(&mut hasher);
            self.hearing.hash(&mut hasher);
            hasher.finish()
        } else {
            0
        }
    }

    fn senses(
        &self,
        my_transform: &TransformComponent,
        ur_pos: &WorldPoint,
        distance: f32,
    ) -> Sense {
        let mut result = Sense::empty();
        let forward = my_transform.forwards();

        if self
            .vision
            .iter()
            .any(|v| v.senses(&forward, &my_transform.position, ur_pos, distance))
        {
            result.insert(Sense::VISION);
        }

        // if self.hearing.iter().any(|h| h.senses(distance)) {
        //     result.insert(Sense::HEARING);
        // }

        result
    }

    pub fn sensed_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.sensed.iter().map(|s| s.entity)
    }

    fn add_sensed_entity(&mut self, e: Entity, how: Sense) {
        debug_assert!(!how.is_empty());

        if let Some(existing) = self.sensed.iter_mut().find(|s| s.entity == e) {
            // renew decay
            existing.decay = SENSE_DECAY;
        } else {
            // new entity
            self.sensed.push(SensedEntity {
                entity: e,
                how,
                decay: SENSE_DECAY,
            })
        }
    }

    fn decay_sensed_entities(&mut self) {
        // decrement
        self.sensed
            .iter_mut()
            .for_each(|s| s.decay = s.decay.saturating_sub(1));

        // sort expired to the end
        self.sensed.sort_unstable_by_key(|s| s.decay > 0);

        if let Some(first_expired) = self.sensed.iter().position(|s| s.decay == 0) {
            let count = self.sensed.drain(first_expired..).count();
            trace!("removed {count} expired sensed entities", count = count);
        }
    }
}

impl Debug for SensesComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "Senses(hearing={:?}, vision={:?})",
            self.hearing, self.vision
        )
    }
}

impl<V: Value> ComponentTemplate<V> for MagicalSenseComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        #[derive(Deserialize)]
        struct DeVision {
            length: f32,
            /// Degrees
            angle: f32,

            #[serde(default)]
            angle_offset: f32,
        }

        // #[derive(Deserialize)]
        // struct DeHearing {
        //     radius: f32,
        // }

        let vision: DeVision = values.get("vision").and_then(|v| v.into_type())?;
        // let hearing: DeHearing = values.get("hearing").and_then(|v| v.into_type())?;

        Ok(Box::new(Self {
            vision: VisionCone {
                length: vision.length,
                angle: deg(vision.angle).into(),
                angle_offset: deg(vision.angle_offset).into(),
            },
            // hearing: HearingSphere {
            //     radius: hearing.radius,
            // },
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone()).with(SensesComponent::default())
    }
}

register_component_template!("senses", MagicalSenseComponent);
