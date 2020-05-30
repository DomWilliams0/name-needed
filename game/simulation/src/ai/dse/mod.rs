use ai::{AiBox, Dse};
pub use items::{FindLocalFoodDse, UseHeldFoodDse};
pub use wander::WanderDse;

use crate::ai::AiContext;

mod items;
mod wander;

macro_rules! dse {
    ($dse:expr) => {
        AiBox::new($dse) as Box<dyn Dse<AiContext>>
    };
}

pub fn human_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
    vec![
        dse!(WanderDse),
        dse!(UseHeldFoodDse),
        dse!(FindLocalFoodDse),
    ]
    .into_iter()
}
