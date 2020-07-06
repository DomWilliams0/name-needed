use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};
use common::*;

declare_entity_metric!(AI_HUNGER, "ai_hunger", "Hunger level");

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

    #[cfg(feature = "metrics")]
    fn log_metric(&self, entity: &str, value: f32) {
        entity_metric!(AI_HUNGER, entity, value);
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::consideration::HungerConsideration;
    use crate::ai::AiBlackboard;
    use ai::{Consideration, InputCache};
    use common::NormalizedFloat;
    use std::mem::MaybeUninit;

    #[test]
    fn hunger() {
        // initialize blackboard with only what we want
        let blackboard = MaybeUninit::<AiBlackboard>::zeroed();
        let mut blackboard = unsafe { blackboard.assume_init() };
        let mut cache = InputCache::default();

        let hunger = HungerConsideration;

        blackboard.hunger = NormalizedFloat::one();
        let score_when_full = hunger
            .curve()
            .evaluate(hunger.consider(&mut blackboard, &mut cache));
        cache.reset();

        blackboard.hunger = NormalizedFloat::new(0.2);
        let score_when_hungry = hunger
            .curve()
            .evaluate(hunger.consider(&mut blackboard, &mut cache));
        cache.reset();

        blackboard.hunger = NormalizedFloat::new(0.01);
        let score_when_empty = hunger
            .curve()
            .evaluate(hunger.consider(&mut blackboard, &mut cache));
        cache.reset();

        assert!(
            score_when_hungry > score_when_full,
            "less fuel in hunger -> more hungry -> higher score"
        );

        assert!(score_when_full.value() <= 0.1);
        assert!(score_when_empty.value() >= 1.0);
    }
}
