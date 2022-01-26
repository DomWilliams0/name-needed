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
    use crate::ai::system::Species;
    use crate::ai::{AiBlackboard, AiComponent, SharedBlackboard};
    use crate::ecs::Builder;
    use crate::{ComponentWorld, EcsWorld, WorldPosition};
    use ai::{Consideration, InputCache};
    use common::bumpalo::Bump;
    use common::NormalizedFloat;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct NoLeaksGuard(*mut EcsWorld);

    impl Drop for NoLeaksGuard {
        fn drop(&mut self) {
            // safety: ptr came from leaked boxes
            unsafe {
                let _ = Box::from_raw(self.0);
            }
        }
    }

    fn dummy_blackboard() -> (AiBlackboard<'static>, NoLeaksGuard) {
        let world = Box::leak(Box::new(EcsWorld::new()));
        let ai = Box::leak(Box::new(AiComponent::with_species(&Species::Human)));
        let shared = Rc::new(RefCell::new(SharedBlackboard {
            area_link_cache: Default::default(),
        }));

        let guard = NoLeaksGuard(world as *mut _);

        let blackboard = AiBlackboard {
            entity: world.create_entity().build().into(),
            accessible_position: WorldPosition::new(1, 2, 3.into()),
            position: Default::default(),
            hunger: None,
            inventory: None,
            society: None,
            ai,
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
        let alloc = Bump::new();
        let mut cache = InputCache::new(&alloc);

        let hunger = HungerConsideration;

        blackboard.hunger = Some(NormalizedFloat::one());
        let score_when_full = hunger
            .curve()
            .evaluate(hunger.consider(&mut blackboard, &mut cache));
        cache = InputCache::new(&alloc);

        blackboard.hunger = Some(NormalizedFloat::new(0.2));
        let score_when_hungry = hunger
            .curve()
            .evaluate(hunger.consider(&mut blackboard, &mut cache));
        cache = InputCache::new(&alloc);

        blackboard.hunger = Some(NormalizedFloat::new(0.01));
        let score_when_empty = hunger
            .curve()
            .evaluate(hunger.consider(&mut blackboard, &mut cache));

        assert!(
            score_when_hungry > score_when_full,
            "less fuel in hunger -> more hungry -> higher score"
        );

        assert!(score_when_full.value() <= 0.1);
        assert!(score_when_empty.value() >= 1.0);
    }
}
