use crate::ai::{AiContext, AiInput};
use ai::{Consideration, ConsiderationParameter, Context, Curve};
use unit::world::{WorldPoint, WorldPosition};

use crate::ai::input::BlockTypeMatch;

pub enum Proximity {
    /// e.g. a job that can be walked to
    Walkable,
    /// Very close, probably in view
    Nearby,
    /// Distance, not squared
    Custom(f32),
}

pub struct MyProximityToConsideration {
    pub target: WorldPoint,

    /// Anything further than this will be 0.0
    pub proximity: Proximity,
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
        // TODO take mobility into account, e.g. more injured = prefer closer
        ConsiderationParameter::Range {
            min: 0.25,
            max: self.proximity.distance().powi(2),
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

impl Proximity {
    fn distance(&self) -> f32 {
        match self {
            Proximity::Walkable => 400.0,
            Proximity::Nearby => 40.0,
            Proximity::Custom(f) => *f,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proximity_consideration() {
        let c = MyProximityToConsideration {
            target: WorldPoint::new_unchecked(0.0, 0.0, 0.0),
            proximity: Proximity::Custom(5.0),
        };

        let value = |val| {
            let x = c.consider_input(val);
            c.curve().evaluate(x).value()
        };

        let very_far = value(60.0);
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
