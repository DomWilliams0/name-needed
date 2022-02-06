use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::consideration::MyProximityToTargetConsideration;
use crate::ai::{AiContext, AiInput, AiTarget};

pub struct MyProximityToConsideration(pub AiTarget);

impl Consideration<AiContext> for MyProximityToConsideration {
    fn curve(&self) -> Curve {
        MyProximityToTargetConsideration.curve()
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::MyDistance2To(self.0)
    }

    fn parameter(&self) -> ConsiderationParameter {
        MyProximityToTargetConsideration.parameter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unit::world::WorldPoint;

    /// Takes raw distance, returns score 0-1
    fn value(dist: f32) -> f32 {
        let c =
            MyProximityToConsideration(AiTarget::Point(WorldPoint::new_unchecked(0.0, 0.0, 0.0)));
        let dist2 = dist * dist;
        let x = c.consider_input(dist2);
        c.curve().evaluate(x).value()
    }

    #[test]
    fn proximity_consideration() {
        let very_far = dbg!(value(60.0));
        let far = dbg!(value(10.0));
        let closer = dbg!(value(7.0));
        let closerrr = dbg!(value(3.0));
        let arrived = dbg!(value(0.1));

        assert!(very_far <= 0.0);
        assert!(far > very_far);
        assert!(closer > far);
        assert!(arrived > closer);
        assert!(closerrr > closer);
        assert!(arrived > closerrr);
        assert!(arrived >= 1.0);
    }
}
