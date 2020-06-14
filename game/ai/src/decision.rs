use common::*;

use crate::consideration::InputCache;
use crate::{AiBox, Consideration, Context};

#[derive(Copy, Clone)]
pub enum DecisionWeight {
    Idle,
    Normal,
    /// This normally follows another decision and is disabled by a switch - once the switch toggles
    /// this is more likely to be chosen
    Dependent,
    BasicNeeds,
    Emergency,
}

pub trait Dse<C: Context> {
    fn name(&self) -> &str;
    /// TODO pooled vec/slice rather than Vec each time
    fn considerations(&self) -> Vec<AiBox<dyn Consideration<C>>>;
    fn weight(&self) -> DecisionWeight;
    fn action(&self, blackboard: &mut C::Blackboard) -> C::Action;

    fn score(
        &self,
        blackboard: &mut C::Blackboard,
        input_cache: &mut InputCache<C>,
        bonus: f32,
    ) -> f32 {
        let mut final_score = bonus;

        let considerations = self.considerations();
        let modification_factor = 1.0 - (1.0 / considerations.len() as f32);
        for c in considerations.iter() {
            // TODO optimization: dont consider all considerations every time

            let score = c.consider(blackboard, input_cache).value();

            // compensation factor balances overall drop when multiplying multiple floats by
            // taking into account the number of considerations
            let make_up_value = (1.0 - score) * modification_factor;
            let compensated_score = score + (make_up_value * score);
            debug_assert!(compensated_score <= 1.0);

            let evaluated_score = c
                .curve()
                .evaluate(NormalizedFloat::new(compensated_score))
                .value();
            trace!(
                "consideration '{}' raw value is {:?} and scored {:?}",
                c.name(),
                score,
                evaluated_score
            );

            #[cfg(feature = "logging")]
            {
                use crate::Blackboard;
                c.log_metric(&blackboard.entity(), evaluated_score);
            }

            final_score *= evaluated_score;
        }

        final_score * self.weight().multiplier()
    }
}

impl DecisionWeight {
    pub fn multiplier(self) -> f32 {
        match self {
            DecisionWeight::Idle => 1.0,
            DecisionWeight::Normal => 2.0,
            DecisionWeight::Dependent => 2.5,
            DecisionWeight::BasicNeeds => 3.5,
            DecisionWeight::Emergency => 4.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::consideration::InputCache;
    use crate::intelligence::Smarts;
    use crate::{
        AiBox, Blackboard, Consideration, ConsiderationParameter, Context, Curve, DecisionWeight,
        Dse, Input, Intelligence, IntelligentDecision,
    };
    use matches::assert_matches;
    use std::fmt::Debug;

    // TODO put this in common test utils?

    #[derive(Eq, PartialEq, Clone, Debug)]
    enum TestAction {
        Nop,
        Eat,
        CancelExistence,
    }

    impl Default for TestAction {
        fn default() -> Self {
            TestAction::Nop
        }
    }

    #[derive(Clone, Hash, Eq, PartialEq)]
    enum TestInput {
        MyHunger,
        DoAnythingElse,
    }

    impl Input<TestContext> for TestInput {
        fn get(&self, blackboard: &mut <TestContext as Context>::Blackboard) -> f32 {
            match self {
                TestInput::MyHunger => blackboard.my_hunger,
                TestInput::DoAnythingElse => 0.001,
            }
        }
    }

    struct TestBlackboard {
        my_hunger: f32,
    }

    impl Blackboard for TestBlackboard {
        #[cfg(feature = "logging")]
        fn entity(&self) -> String {
            String::new()
        }
    }

    struct TestContext;

    impl Context for TestContext {
        type Blackboard = TestBlackboard;
        type Input = TestInput;
        type Action = TestAction;
    }

    struct MyHungerConsideration;

    impl Consideration<TestContext> for MyHungerConsideration {
        fn curve(&self) -> Curve {
            Curve::Linear(1.0, 0.0)
        }

        fn input(&self) -> <TestContext as Context>::Input {
            TestInput::MyHunger
        }

        fn parameter(&self) -> ConsiderationParameter {
            ConsiderationParameter::Nop
        }

        #[cfg(feature = "logging")]
        fn log_metric(&self, _: &str, _: f32) {}
    }

    struct CancelExistenceConsideration;

    impl Consideration<TestContext> for CancelExistenceConsideration {
        fn curve(&self) -> Curve {
            Curve::Linear(0.0, 0.0)
        }

        fn input(&self) -> <TestContext as Context>::Input {
            TestInput::DoAnythingElse
        }

        fn parameter(&self) -> ConsiderationParameter {
            ConsiderationParameter::Range {
                min: 0.0,
                max: 20.0,
            }
        }

        #[cfg(feature = "logging")]
        fn log_metric(&self, _: &str, _: f32) {}
    }

    struct EatDse;

    impl Dse<TestContext> for EatDse {
        fn name(&self) -> &str {
            "Eat"
        }

        fn considerations(&self) -> Vec<AiBox<dyn Consideration<TestContext>>> {
            vec![AiBox::new(MyHungerConsideration)]
        }

        fn weight(&self) -> DecisionWeight {
            DecisionWeight::Normal
        }

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::Eat
        }
    }

    struct BadDse;

    impl Dse<TestContext> for BadDse {
        fn name(&self) -> &str {
            unimplemented!()
        }

        fn considerations(&self) -> Vec<AiBox<dyn Consideration<TestContext>>> {
            vec![AiBox::new(CancelExistenceConsideration)]
        }

        fn weight(&self) -> DecisionWeight {
            DecisionWeight::Emergency
        }

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::CancelExistence
        }
    }

    #[test]
    fn score() {
        let mut blackboard = TestBlackboard { my_hunger: 0.5 };
        let mut cache = InputCache::default();

        assert!(EatDse.score(&mut blackboard, &mut cache, 1.0) > 0.9);
        assert!(BadDse.score(&mut blackboard, &mut cache, 1.0) < 0.1);

        let smarts = Smarts::new(
            vec![
                AiBox::new(EatDse) as AiBox<dyn Dse<TestContext>>,
                AiBox::new(BadDse) as AiBox<dyn Dse<TestContext>>,
            ]
            .into_iter(),
        )
        .unwrap();

        let mut intelligence = Intelligence::new(smarts);
        if let IntelligentDecision::New { action, .. } = intelligence.choose(&mut blackboard) {
            assert_matches!(action, TestAction::Eat); // phew
        } else {
            panic!()
        }
    }
}
