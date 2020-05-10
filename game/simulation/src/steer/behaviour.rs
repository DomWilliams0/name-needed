use common::*;
use unit::world::WorldPosition;

use crate::steer::context::InterestsContextMap;
use crate::TransformComponent;

#[derive(Debug)]
pub enum SteeringBehaviour {
    Nop(Nop),
    Seek(Seek),
    // TODO wander?
}

/// When steering is complete
pub enum SteeringResult {
    /// Not yet complete
    Ongoing,

    /// Complete
    Finished,
}

impl Default for SteeringBehaviour {
    fn default() -> Self {
        // TODO wander? depends on steer, put in trait
        Self::Nop(Nop)
    }
}

impl SteeringBehaviour {
    pub fn seek<P: Into<WorldPosition>>(target: P) -> Self {
        SteeringBehaviour::Seek(Seek::with_target(target.into()))
    }

    pub fn tick(
        &mut self,
        transform: &TransformComponent,
        interests: &mut InterestsContextMap,
    ) -> SteeringResult {
        match self {
            SteeringBehaviour::Nop(behaviour) => behaviour.tick(transform, interests),
            SteeringBehaviour::Seek(behaviour) => behaviour.tick(transform, interests),
        }
    }

    pub fn is_nop(&self) -> bool {
        matches!(self, SteeringBehaviour::Nop(_))
    }
}

trait DoASteer {
    fn tick(
        &mut self,
        transform: &TransformComponent,
        interests: &mut InterestsContextMap,
    ) -> SteeringResult;
}

// stand still
#[derive(Default, Debug)]
pub struct Nop;

impl DoASteer for Nop {
    fn tick(&mut self, _: &TransformComponent, _: &mut InterestsContextMap) -> SteeringResult {
        SteeringResult::Ongoing
    }
}

/// Seek to and stop when reached with no slowdown
#[derive(Debug)]
pub struct Seek {
    target: WorldPosition,

    // used to detect if the point has been passed
    original_delta: Option<Vector2>,
    original_sign: Option<bool>,
}

impl Seek {
    pub fn with_target(target: WorldPosition) -> Self {
        Self {
            target,
            original_delta: None,
            original_sign: None,
        }
    }
}

impl DoASteer for Seek {
    fn tick(
        &mut self,
        transform: &TransformComponent,
        interests: &mut InterestsContextMap,
    ) -> SteeringResult {
        // ignore z direction, assume the target is accessible and accurate.
        // round to use block positions instead of points so the general direction to the target
        // is used rather than point-to-point to the centre every time.
        let tgt = Vector2::new(self.target.0 as f32, self.target.1 as f32);
        let pos = Vector2::new(transform.position.0.floor(), transform.position.1.floor());
        let delta = tgt - pos;

        // use exact position for distance check though
        if (tgt - Vector2::from(transform.position)).magnitude2() <= 1.8f32 {
            return SteeringResult::Finished;
        }

        match (&self.original_delta, &self.original_sign) {
            (None, _) => {
                // first tick
                self.original_delta = Some(delta);
            }
            (Some(original_delta), None) => {
                // second tick
                self.original_sign = Some(original_delta.dot(delta).is_sign_positive());
            }
            (Some(original_delta), Some(original_sign)) => {
                // any other tick
                let sign = original_delta.dot(delta).is_sign_positive();
                if sign != *original_sign {
                    // passed the point, seek is over
                    return SteeringResult::Finished;
                }
            }
        }

        // keep seeking directly towards at full speed
        // TODO use WorldPosition instead so aims for block instead of centre every time
        let angle = delta.angle(AXIS_FWD.truncate());
        interests.write_interest(angle, 1.0);
        SteeringResult::Ongoing
    }
}
