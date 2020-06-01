//! Infinite axis utility system

pub use consideration::{Consideration, ConsiderationParameter, Curve, InputCache};
pub use decision::{DecisionWeight, Dse};
pub use intelligence::{Intelligence, IntelligentDecision, Smarts};
use std::hash::Hash;

mod consideration;
mod decision;
mod intelligence;

pub trait Context: Sized {
    type Blackboard: Blackboard;
    type Input: Input<Self>;
    type Action: Default + Eq + Clone;
}

pub trait Input<C: Context>: Hash + Clone + Eq {
    fn get(&self, blackboard: &mut C::Blackboard) -> f32;
}

pub trait Blackboard {
    #[cfg(feature = "logging")]
    fn entity(&self) -> String;
}

pub type AiBox<T> = Box<T>;
