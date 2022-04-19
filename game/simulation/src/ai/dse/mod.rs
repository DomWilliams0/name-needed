pub use items::*;
pub use obey_divine_command::*;
pub use species::*;
pub use wander::*;

pub use self::world::*;

mod interact;
mod items;
mod obey_divine_command;
mod wander;
mod world;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum AdditionalDse {
    DivineCommand,
}

// TODO species concept for ai is temporary
pub mod species {
    use ai::{AiBox, Dse};

    use crate::ai::dse::interact::StayCloseToHerdDse;
    use crate::ai::AiContext;
    use crate::dse;

    use super::*;

    pub fn human_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
        [
            dse!(WanderDse),
            dse!(EatHeldFoodDse),
            dse!(FindLocalFoodDse),
        ]
        .into_iter()
    }

    pub fn dog_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
        [dse!(WanderDse)].into_iter()
    }

    pub fn sheep_dses() -> impl Iterator<Item = AiBox<dyn Dse<AiContext>>> {
        [dse!(WanderDse), dse!(StayCloseToHerdDse)].into_iter()
    }
}
