use ai::{AiBox, Dse};
pub use dev::ObeyDivineCommandDse;
pub use items::{FindLocalFoodDse, UseHeldFoodDse};
pub use movement::WanderDse;

use crate::ai::AiContext;

mod dev;
mod items;
mod movement;

macro_rules! dse {
    ($dse:expr) => {
        AiBox::new($dse) as Box<dyn Dse<AiContext>>
    };
}

pub fn human_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
    vec![
        dse!(ObeyDivineCommandDse),
        dse!(WanderDse),
        dse!(UseHeldFoodDse),
        dse!(FindLocalFoodDse),
    ]
    .into_iter()
}
