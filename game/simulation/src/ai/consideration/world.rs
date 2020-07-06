use crate::ai::{AiContext, AiInput};
use ai::{Consideration, ConsiderationParameter, Context, Curve};
use unit::world::WorldPosition;

use crate::ai::input::BlockTypeMatch;

pub struct MyProximityToConsideration {
    pub target: WorldPosition,

    /// Anything further than this radius is 0.0
    pub max_distance: f32,
}

pub struct BlockTypeMatchesConsideration(pub WorldPosition, pub BlockTypeMatch);

impl Consideration<AiContext> for MyProximityToConsideration {
    fn curve(&self) -> Curve {
        Curve::SquareRoot(1.0, -1.0, 1.0)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::MyDistance2To(self.target)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Range {
            min: 0.25,
            max: self.max_distance,
        }
    }
}

impl Consideration<AiContext> for BlockTypeMatchesConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::BlockTypeMatches(self.0, self.1)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proximity_consideration() {
        let c = MyProximityToConsideration {
            target: (0, 0, 0).into(),
            max_distance: 5.0,
        };

        let value = |val| {
            let x = c.consider_input(val);
            c.curve().evaluate(x).value()
        };

        let very_far = value(10.0);
        let far = value(4.0);
        let closer = value(2.0);
        let closerrr = value(0.5);
        let arrived = value(0.1);

        assert!(very_far <= 0.0);
        assert!(far > very_far);
        assert!(closer > far);
        assert!(arrived > closer);
        assert!(closerrr > closer);
        assert!(arrived > closerrr);
        assert!(arrived >= 1.0);
    }
}
