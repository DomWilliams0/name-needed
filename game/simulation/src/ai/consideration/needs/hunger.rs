use ai::{Consideration, ConsiderationParameter, Context, Curve};
use common::*;

use crate::ai::{AiContext, AiInput};

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
    use std::cell::RefCell;
    use std::rc::Rc;

    use ai::{Consideration, InputCache};
    use common::bumpalo::Bump;
    use common::NormalizedFloat;
    use unit::food::{Metabolism, Nutrition};
    use unit::world::WorldPoint;

    use crate::ai::consideration::HungerConsideration;
    use crate::ai::system::Species;
    use crate::ai::{AiBlackboard, AiComponent, SharedBlackboard};
    use crate::ecs::Builder;
    use crate::needs::food::FoodInterest;
    use crate::{ComponentWorld, EcsWorld, HungerComponent, TransformComponent, WorldPosition};

    struct NoLeaksGuard(*mut EcsWorld, *mut TransformComponent);

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
        let transform = Box::leak(Box::new(TransformComponent::new(
            WorldPoint::new_unchecked(1.0, 2.0, 3.0),
        )));
        let ai = Box::leak(Box::new(AiComponent::with_species(&Species::Human)));
        let shared = Rc::new(RefCell::new(SharedBlackboard {
            area_link_cache: Default::default(),
        }));

        let guard = NoLeaksGuard(world as *mut _, transform as *mut _);

        let blackboard = AiBlackboard {
            entity: world.create_entity().build().into(),
            transform,
            inventory: None,
            society: None,
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

        let _ = blackboard.world.add_now(
            blackboard.entity,
            HungerComponent::new(
                Nutrition::new(100),
                Metabolism::new(0.5).unwrap(),
                FoodInterest::empty(),
            ),
        );
        macro_rules! set_hunger {
            ($f:expr) => {{
                let mut comp = blackboard
                    .world
                    .component_mut::<HungerComponent>(blackboard.entity)
                    .unwrap();
                comp.hunger_mut().set_satiety(NormalizedFloat::new($f));
            }};
        }

        set_hunger!(1.0);
        let score_when_full =
            hunger
                .curve()
                .evaluate(hunger.consider(&mut blackboard, None, &mut cache));
        cache = InputCache::new(&alloc);

        set_hunger!(0.2);
        let score_when_hungry =
            hunger
                .curve()
                .evaluate(hunger.consider(&mut blackboard, None, &mut cache));
        cache = InputCache::new(&alloc);

        set_hunger!(0.01);
        let score_when_empty =
            hunger
                .curve()
                .evaluate(hunger.consider(&mut blackboard, None, &mut cache));

        assert!(
            score_when_hungry > score_when_full,
            "less fuel in hunger -> more hungry -> higher score"
        );

        assert!(score_when_full.value() <= 0.1);
        assert!(score_when_empty.value() >= 1.0);
    }
}
