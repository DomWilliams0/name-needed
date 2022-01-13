//! Infinite axis utility system

pub use consideration::{Consideration, ConsiderationParameter, Curve};
pub use decision::{Considerations, DecisionWeightType, Dse, WeightedDse};
pub use intelligence::{DecisionSource, Intelligence, IntelligentDecision, Smarts};

use common::bumpalo;
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

pub(crate) fn pretty_type_name(name: &str) -> &str {
    let split_idx = name.rfind(':').map(|i| i + 1).unwrap_or(0);
    &name[split_idx..]
}

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

    pub struct EatDse;

    impl Dse<TestContext> for EatDse {
        fn name(&self) -> &'static str {
            "Eat"
        }

        fn considerations(&self, out: &mut Considerations<TestContext>) {
            out.add(MyHungerConsideration);
        }

        fn weight_type(&self) -> DecisionWeightType {
            DecisionWeightType::Normal
        }

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::Eat
        }
    }

    pub struct BadDse;

    impl Dse<TestContext> for BadDse {
        fn name(&self) -> &'static str {
            "Bad"
        }

        fn considerations(&self, out: &mut Considerations<TestContext>) {
            out.add(CancelExistenceConsideration);
        }

        fn weight_type(&self) -> DecisionWeightType {
            DecisionWeightType::Emergency
        }

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::CancelExistence
        }
    }

    pub struct EmergencyDse;

    impl Dse<TestContext> for EmergencyDse {
        fn name(&self) -> &'static str {
            "Emergency"
        }

        fn considerations(&self, out: &mut Considerations<TestContext>) {
            out.add(AlwaysWinConsideration);
        }

        fn weight_type(&self) -> DecisionWeightType {
            DecisionWeightType::AbsoluteOverride
        }

        fn action(&self, _: &mut TestBlackboard) -> TestAction {
            TestAction::CancelExistence // sorry
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_type_names() {
        assert_eq!(pretty_type_name("this::is::my::type::Lmao"), "Lmao");
        assert_eq!(pretty_type_name("boop"), "boop");
        assert_eq!(pretty_type_name("malformed:"), "");
        assert_eq!(pretty_type_name(":malformed"), "malformed");
    }
}
