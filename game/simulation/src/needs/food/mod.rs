mod component;
mod flavour;
mod system;

pub use component::{BeingEatenComponent, Fuel, HungerComponent};
pub use system::{EatingSystem, FoodEatingError, HungerSystem};

pub use flavour::{FoodFlavours, FoodInterest};
