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
    use crate::ai::{AiBlackboard, SharedBlackboard};
    use crate::ecs::Builder;
    use crate::{ComponentWorld, EcsWorld, WorldPosition};
    use ai::{Consideration, InputCache};
    use common::NormalizedFloat;

    struct NoLeaksGuard(*mut EcsWorld, *mut SharedBlackboard);

    impl Drop for NoLeaksGuard {
        fn drop(&mut self) {
            // safety: ptrs came from leaked boxes
            unsafe {
                let _ = Box::from_raw(self.0);
                let _ = Box::from_raw(self.1);
            }
        }
    }

    fn dummy_blackboard() -> (AiBlackboard<'static>, NoLeaksGuard) {
        let world = Box::leak(Box::new(EcsWorld::new()));
        let shared = Box::leak(Box::new(SharedBlackboard {
            area_link_cache: Default::default(),
        }));

        let guard = NoLeaksGuard(world as *mut _, shared as *mut _);

        let blackboard = AiBlackboard {
            entity: world.create_entity().build(),
            accessible_position: WorldPosition::new(1, 2, 3.into()),
            position: Default::default(),
            hunger: None,
            inventory: None,
            inventory_search_cache: Default::default(),
            local_area_search_cache: Default::default(),
            world,
            shared,
        };

        (blackboard, guard)
    }

    #[test]
    fn hunger() {
        // initialize blackboard with only what we want
        let (mut blackboard, _guard) = dummy_blackboard();
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
