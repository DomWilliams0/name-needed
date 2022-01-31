use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};

// TODO take into account general world/society size? need some scale
pub struct MyProximityToTargetConsideration;

impl Consideration<AiContext> for MyProximityToTargetConsideration {
    fn curve(&self) -> Curve {
        Curve::SquareRoot(1.05, -1.05, 1.0)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::MyDistance2ToTarget
    }

    fn parameter(&self) -> ConsiderationParameter {
        // TODO take mobility into account, e.g. more injured = prefer closer
        const MAX_DISTANCE: f32 = 50.0;
        ConsiderationParameter::Range {
            min: 0.25,
            max: MAX_DISTANCE.powi(2),
        }
    }
}
