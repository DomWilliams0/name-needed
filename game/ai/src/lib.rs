//! Infinite axis utility system
#![allow(clippy::type_complexity)]

pub use consideration::{Consideration, ConsiderationParameter, Considerations, Curve};
pub use context::{Action, AiBox, Blackboard, Context, Input};
pub use decision::{DecisionWeight, Dse, TargetOutput, Targets, WeightedDse};
pub use intelligence::{
    DecisionProgress, DecisionSource, DseSkipper, InitialChoice, InputCache, Intelligence,
    IntelligentDecision, Smarts, StreamDseScorer,
};

mod consideration;
mod context;
mod decision;
mod intelligence;

#[cfg(test)]
mod test_utils {
    use super::*;

    #[derive(Eq, PartialEq, Clone, Debug, Hash)]
    pub enum TestAction {
        Nop,
        Eat,
        CancelExistence,
        Attack(u32),
    }

    impl Action for TestAction {
        type Arg = ();

        fn cmp(&self, other: &Self, _: &Self::Arg) -> bool {
            self == other
        }
    }

    impl Default for TestAction {
        fn default() -> Self {
            TestAction::Nop
        }
    }

    #[derive(Clone, Hash, Eq, PartialEq)]
    pub enum TestInput {
        MyHunger,
        DoAnythingElse,
        One,
        /// out of 100
        Constant(u32),
        IsTargetFive,
    }

    impl Input<TestContext> for TestInput {
        fn get(
            &self,
            blackboard: &mut <TestContext as Context>::Blackboard,
            target: Option<&u32>,
        ) -> f32 {
            match self {
                TestInput::MyHunger => blackboard.my_hunger,
                TestInput::DoAnythingElse => 0.001,
                TestInput::One => 1.0,
                TestInput::Constant(c) => (*c as f32) / 100.0,
                TestInput::IsTargetFive => {
                    if target.copied() == Some(5) {
                        1.0
                    } else {
                        0.0
                    }
                }
            }
        }
    }

    #[derive(Clone, Default)]
    pub struct TestBlackboard {
        pub my_hunger: f32,
        pub targets: Vec<u32>,
    }

    impl Blackboard for TestBlackboard {
        #[cfg(feature = "logging")]
        fn entity(&self) -> String {
            String::new()
        }
    }

    #[derive(Debug)]
    pub struct TestContext;

    impl Context for TestContext {
        type Blackboard = TestBlackboard;
        type Input = TestInput;
        type Action = TestAction;
        type AdditionalDseId = u32;
        type StreamDseExtraData = u32;
        type DseTarget = u32;
    }

    pub struct MyHungerConsideration;

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
    }

    pub struct CancelExistenceConsideration;

    impl Consideration<TestContext> for CancelExistenceConsideration {
        fn curve(&self) -> Curve {
            Curve::Linear(0.0, 0.0) // never
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
    }

    pub struct AlwaysWinConsideration;

    impl Consideration<TestContext> for AlwaysWinConsideration {
        fn curve(&self) -> Curve {
            Curve::Identity
        }

        fn input(&self) -> <TestContext as Context>::Input {
            TestInput::One
        }

        fn parameter(&self) -> ConsiderationParameter {
            ConsiderationParameter::Nop
        }
    }

    /// Out of 100
    pub struct ConstantConsideration(pub u32);

    impl Consideration<TestContext> for ConstantConsideration {
        fn curve(&self) -> Curve {
            Curve::Identity
        }

        fn input(&self) -> TestInput {
            TestInput::Constant(self.0)
        }

        fn parameter(&self) -> ConsiderationParameter {
            ConsiderationParameter::Nop
        }
    }

    #[derive(Clone, PartialEq, Eq, Hash)]
    pub struct EatDse;

    impl Dse<TestContext> for EatDse {
        fn considerations(&self, out: &mut Considerations<TestContext>) {
            out.add(MyHungerConsideration);
        }

        fn weight(&self) -> DecisionWeight {
            DecisionWeight::Normal
        }

        fn action(&self, blackboard: &mut TestBlackboard, target: Option<u32>) -> TestAction {
            TestAction::Eat
        }
    }

    #[derive(Clone, PartialEq, Eq, Hash)]
    pub struct BadDse;

    impl Dse<TestContext> for BadDse {
        fn considerations(&self, out: &mut Considerations<TestContext>) {
            out.add(CancelExistenceConsideration);
        }

        fn weight(&self) -> DecisionWeight {
            DecisionWeight::Emergency
        }

        fn action(&self, blackboard: &mut TestBlackboard, target: Option<u32>) -> TestAction {
            TestAction::CancelExistence
        }
    }

    #[derive(Clone, PartialEq, Eq, Hash)]
    pub struct EmergencyDse;

    impl Dse<TestContext> for EmergencyDse {
        fn considerations(&self, out: &mut Considerations<TestContext>) {
            out.add(AlwaysWinConsideration);
        }

        fn weight(&self) -> DecisionWeight {
            DecisionWeight::AbsoluteOverride
        }

        fn action(&self, blackboard: &mut TestBlackboard, target: Option<u32>) -> TestAction {
            TestAction::CancelExistence // sorry
        }
    }
}
