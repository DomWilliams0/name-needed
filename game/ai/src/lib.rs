//! Infinite axis utility system

pub use consideration::{Consideration, ConsiderationParameter, Curve, InputCache};
pub use decision::{DecisionWeight, Dse};
pub use intelligence::{DecisionSource, Intelligence, IntelligentDecision, Smarts};
use std::fmt::Debug;
use std::hash::Hash;

mod consideration;
mod decision;
mod intelligence;

pub trait Context: Sized {
    type Blackboard: Blackboard;
    type Input: Input<Self>;
    type Action: Default + Eq + Clone;
    type AdditionalDseId: Hash + Eq + Clone + Debug;
}

pub trait Input<C: Context>: Hash + Clone + Eq {
    fn get(&self, blackboard: &mut C::Blackboard) -> f32;
}

pub trait Blackboard {
    #[cfg(feature = "logging")]
    fn entity(&self) -> String;
}

pub type AiBox<T> = Box<T>;

#[cfg(test)]
mod test_utils {
    use super::*;

    #[derive(Eq, PartialEq, Clone, Debug)]
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
    }

    impl Input<TestContext> for TestInput {
        fn get(&self, blackboard: &mut <TestContext as Context>::Blackboard) -> f32 {
            match self {
                TestInput::MyHunger => blackboard.my_hunger,
                TestInput::DoAnythingElse => 0.001,
                TestInput::One => 1.0,
            }
        }
    }

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

    pub struct EatDse;

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

    pub struct BadDse;

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

    pub struct EmergencyDse;

    impl Dse<TestContext> for EmergencyDse {
        fn name(&self) -> &str {
            unimplemented!()
        }

        fn considerations(&self) -> Vec<AiBox<dyn Consideration<TestContext>>> {
            vec![AiBox::new(AlwaysWinConsideration)]
        }

        fn weight(&self) -> DecisionWeight {
            DecisionWeight::AbsoluteOverride
        }

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::CancelExistence // sorry
        }
    }
}