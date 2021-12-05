pub use self::world::{BreakBlockDse, BuildDse, GatherMaterialsDse};
use ai::{AiBox, Dse};
pub use dev::ObeyDivineCommandDse;
pub use food::{EatHeldFoodDse, FindLocalFoodDse};
pub use haul::HaulDse;
pub use movement::WanderDse;

use crate::ai::AiContext;
use crate::dse;

mod dev;
mod food;
mod haul;
mod movement;
mod world;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum AdditionalDse {
    DivineCommand,
}

pub fn human_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
    vec![
        dse!(WanderDse),
        dse!(EatHeldFoodDse),
        dse!(FindLocalFoodDse),
    ]
    .into_iter()
}

pub fn dog_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
    vec![dse!(WanderDse)].into_iter()
}
