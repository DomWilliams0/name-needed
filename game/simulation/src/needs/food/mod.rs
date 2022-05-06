mod component;
mod flavour;
mod hunger;
mod system;

pub use component::{BeingEatenComponent, HungerComponent};
pub use system::{EatingSystem, FoodEatingError, HungerSystem};

pub use flavour::{FoodFlavour, FoodFlavours, FoodInterest};
pub use hunger::FoodDescription;
