use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};

pub struct HungerConsideration;

impl Consideration<AiContext> for HungerConsideration {
    fn curve(&self) -> Curve {
        Curve::Exponential(100.0, -1.0, 0.25, 1.0, -0.04)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::Hunger
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop // already normalized
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::consideration::HungerConsideration;
    use crate::ai::Blackboard;
    use ai::Consideration;
    use common::NormalizedFloat;
    use std::mem::MaybeUninit;

    #[test]
    fn hunger() {
        // initialize blackboard with only what we want
        let blackboard = MaybeUninit::<Blackboard>::zeroed();
        let mut blackboard = unsafe { blackboard.assume_init() };

        let hunger = HungerConsideration;

        blackboard.hunger = NormalizedFloat::one();
        let score_when_full = hunger.curve().evaluate(hunger.consider(&mut blackboard));

        blackboard.hunger = NormalizedFloat::new(0.2);
        let score_when_hungry = hunger.curve().evaluate(hunger.consider(&mut blackboard));

        blackboard.hunger = NormalizedFloat::new(0.01);
        let score_when_empty = hunger.curve().evaluate(hunger.consider(&mut blackboard));

        assert!(
            score_when_hungry > score_when_full,
            "less fuel in hunger -> more hungry -> higher score"
        );

        assert!(score_when_full.value() <= 0.1);
        assert!(score_when_empty.value() >= 1.0);
    }
}
