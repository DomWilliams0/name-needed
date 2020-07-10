pub use self::world::BreakBlockDse;
use ai::{AiBox, Dse};
pub use dev::ObeyDivineCommandDse;
pub use items::{FindLocalFoodDse, UseHeldFoodDse};
pub use movement::WanderDse;

use crate::ai::AiContext;
use crate::dse;

mod dev;
mod items;
mod movement;
mod world;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum AdditionalDse {
    DivineCommand,
}

pub fn human_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
    vec![
        dse!(WanderDse),
        dse!(UseHeldFoodDse),
        dse!(FindLocalFoodDse),
    ]
    .into_iter()
}