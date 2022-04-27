mod component;
mod system;

pub use component::{BeingEatenComponent, Fuel, HungerComponent};
pub use system::{EatingSystem, FoodEatingError, HungerSystem};
