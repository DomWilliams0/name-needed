//! Infinite axis utility system

pub use consideration::{Consideration, ConsiderationParameter, Considerations, Curve};
pub use context::{AiBox, Blackboard, Context, Input};
pub use decision::{DecisionWeight, Dse, WeightedDse};
pub use intelligence::{
    DecisionProgress, DecisionSource, DseSkipper, InitialChoice, InputCache, Intelligence,
    IntelligentDecision, Smarts,
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
    }

    impl Input<TestContext> for TestInput {
        fn get(&self, blackboard: &mut <TestContext as Context>::Blackboard) -> f32 {
            match self {
                TestInput::MyHunger => blackboard.my_hunger,
                TestInput::DoAnythingElse => 0.001,
                TestInput::One => 1.0,
                TestInput::Constant(c) => (*c as f32) / 100.0,
            }
        }
    }

    #[derive(Clone)]
    pub struct TestBlackboard {
        pub my_hunger: f32,
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
        type StreamDseExtraData = ();
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

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::Eat
        }

        fn name(&self) -> &'static str {
            "Eat"
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

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::CancelExistence
        }

        fn name(&self) -> &'static str {
            "Bad"
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

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::CancelExistence // sorry
        }

        fn name(&self) -> &'static str {
            "Emergency"
        }
    }
}
