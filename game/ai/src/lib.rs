//! Infinite axis utility system

pub use consideration::{Consideration, ConsiderationParameter, Curve};
pub use decision::{DecisionWeight, Dse};
pub use intelligence::{Intelligence, IntelligentDecision, Smarts};
use std::fmt::Debug;

mod consideration;
mod decision;
mod intelligence;

pub trait Context: Sized {
    type Blackboard: Blackboard;
    type Input: Input<Self>;
    type Action: Default + Eq + Clone;
}

pub trait Input<C: Context> {
    fn get(&self, blackboard: &mut C::Blackboard) -> f32;
}

pub trait Blackboard {
    fn entity(&self) -> &dyn Debug;
}

// TODO pool/slab
pub type AiBox<T> = Box<T>;
