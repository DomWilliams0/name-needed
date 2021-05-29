use common::*;
use unit::world::WorldPoint;

use crate::steer::context::InterestsContextMap;
use crate::TransformComponent;

#[derive(Debug)]
pub enum SteeringBehaviour {
    Stop(Stop),
    Seek(Seek),
}

/// Arrest current movement
#[derive(Default, Debug)]
pub struct Stop;

/// Seek to and stop when reached with no slowdown
#[derive(Debug)]
pub struct Seek {
    target: WorldPoint,
    speed: NormalizedFloat,
}

/// When steering is complete
#[derive(Debug)]
pub enum SteeringResult {
    /// Not yet complete
    Ongoing,

    /// Complete
    Finished,
}

impl Default for SteeringBehaviour {
    fn default() -> Self {
        Self::Stop(Stop)
    }
}

impl SteeringBehaviour {
    pub fn seek<P: Into<WorldPoint>>(target: P, speed: NormalizedFloat) -> Self {
        SteeringBehaviour::Seek(Seek::with_target(target.into(), speed))
    }

    pub fn tick(
        &mut self,
        transform: &TransformComponent,
        bounding_radius: f32,
        interests: &mut InterestsContextMap,
    ) -> SteeringResult {
        match self {
            SteeringBehaviour::Stop(behaviour) => {
                behaviour.tick(transform, bounding_radius, interests)
            }
            SteeringBehaviour::Seek(behaviour) => {
                behaviour.tick(transform, bounding_radius, interests)
            }
        }
    }

    pub fn is_nop(&self) -> bool {
        matches!(self, SteeringBehaviour::Stop(_))
    }
}

trait DoASteer {
    fn tick(
        &mut self,
        transform: &TransformComponent,
        bounding_radius: f32,
        interests: &mut InterestsContextMap,
    ) -> SteeringResult;

    fn register_interest(
        &self,
        direction: Vector2,
        speed: NormalizedFloat,
        interests: &mut InterestsContextMap,
    ) {
        let angle = direction.angle(AXIS_FWD_2);
        interests.write_interest(angle, speed.value());
    }
}

impl DoASteer for Stop {
    fn tick(
        &mut self,
        transform: &TransformComponent,
        _bounding_radius: f32,
        interests: &mut InterestsContextMap,
    ) -> SteeringResult {
        const STOPPED: f32 = 0.04;
        let velocity = transform.velocity;
        if velocity.magnitude2() > STOPPED * STOPPED {
            // apply braking force
            self.register_interest(-velocity, NormalizedFloat::new(0.2), interests);
        }

        SteeringResult::Ongoing
    }
}

impl Seek {
    pub fn with_target(target: WorldPoint, speed: NormalizedFloat) -> Self {
        Self { target, speed }
    }
}

impl DoASteer for Seek {
    fn tick(
        &mut self,
        transform: &TransformComponent,
        bounding_radius: f32,
        interests: &mut InterestsContextMap,
    ) -> SteeringResult {
        // ignore z direction, assume the target is accessible and accurate.
        let tgt = Vector2::from(self.target);
        let pos = Vector2::from(transform.position);

        let distance = pos.distance(tgt);
        let threshold = {
            const MARGIN: f32 = 1.4;
            let radius = bounding_radius + transform.velocity.magnitude();
            radius * MARGIN
        };
        if distance < threshold {
            // we've arrived
            return SteeringResult::Finished;
        }

        let delta = if distance < 1.0 {
            // close enough to use exact direction now
            tgt - pos
        } else {
            // far away still, use block coords so the direction is generally towards the
            // target without being exactly towards the centre of each block
            let block_tgt = Vector2::from(self.target.floored());
            let block_pos = Vector2::from(transform.position.floored());
            block_tgt - block_pos
        };

        // keep seeking towards target
        self.register_interest(delta, self.speed, interests);
        SteeringResult::Ongoing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steer::context::{ContextMap, Direction};
    use crate::steer::Seek;
    use crate::TransformComponent;
    use common::NormalizedFloat;
    use unit::world::WorldPoint;

    #[test]
    fn seek_dynamic_radius() {
        let mut seek = Seek::with_target(
            WorldPoint::new_unchecked(10.0, 0.0, 0.0),
            NormalizedFloat::one(),
        );

        // starts at 0,0 going to 10,0
        let mut transform = TransformComponent::new(WorldPoint::default());
        let mut output = InterestsContextMap::default();

        // first ticks takes us toward
        assert!(matches!(
            seek.tick(&transform, 0.5, &mut output),
            SteeringResult::Ongoing
        ));

        // overshoot, but not in arrival range - should still finish because the direction changed
        // we're halfway there but going fast enough to arrive next tick
        transform.position.modify_x(|_| 5.0);
        transform.velocity.x = 4.8;
        assert!(matches!(
            seek.tick(&transform, 0.5, &mut output),
            SteeringResult::Finished
        ));
    }
    #[test]
    fn seek_arrived_already() {
        let mut seek = Seek::with_target(
            WorldPoint::new_unchecked(10.0, 0.0, 0.0),
            NormalizedFloat::one(),
        );
        let transform = TransformComponent::new(WorldPoint::new_unchecked(9.8, 0.0, 0.0));
        let mut output = InterestsContextMap::default();

        // already arrived
        assert!(matches!(
            seek.tick(&transform, 0.5, &mut output),
            SteeringResult::Finished
        ));
    }

    #[test]
    fn seek_exact_pos() {
        // we are not exactly lined up with the target, and a tiny radius
        let mut seek = Seek::with_target(
            WorldPoint::new_unchecked(10.8, 0.6, 0.0),
            NormalizedFloat::one(),
        );
        let transform = TransformComponent::new(WorldPoint::new_unchecked(0.2, 0.9, 0.0));
        let mut output = ContextMap::default();

        // output should be towards the block
        assert!(matches!(
            seek.tick(&transform, 0.2, output.interests_mut()),
            SteeringResult::Ongoing
        ));
        let (dir, _) = output.resolve();
        assert!(dir
            .0
            .approx_eq(Into::<Rad>::into(Direction::East).0, (f32::EPSILON, 2)));
    }
}
