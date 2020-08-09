use std::collections::HashMap;

use common::*;
pub use input::AiInput;
pub use system::{ActivityComponent, AiComponent, AiSystem};
use unit::world::WorldPoint;

pub use crate::ai::activity::{AiAction, ItemsToPickUp};
use crate::ai::dse::AdditionalDse;
use crate::ai::input::LocalAreaSearch;
use crate::ecs::{EcsWorld, Entity};
use crate::item::{InventoryComponent, ItemFilter, ItemReference};
pub use dev::{DivineCommandCompletionSystem, DivineCommandComponent};
use world::WorldArea;

mod activity;
mod consideration;
mod dev;
pub mod dse;
mod input;
mod system;

pub struct AiContext;

impl ai::Context for AiContext {
    /// TODO ideally this would use ai::Context<'a> to represent the AI tick lifetime: https://github.com/rust-lang/rust/issues/44265
    type Blackboard = AiBlackboard<'static>;
    type Input = AiInput;
    type Action = AiAction;
    type AdditionalDseId = AdditionalDse;
}

/// 'a: only as long as this AI tick
pub struct AiBlackboard<'a> {
    pub entity: Entity,
    pub position: WorldPoint,
    pub hunger: NormalizedFloat,
    pub inventory: Option<&'a InventoryComponent>,
    pub inventory_search_cache: HashMap<ItemFilter, ItemReference>,

    /// Value is (max distance, results), so smaller ranges can reuse results of bigger ranges
    pub local_area_search_cache: HashMap<ItemFilter, (u32, LocalAreaSearch)>,

    // For fetching other components
    pub world: &'a EcsWorld,
    pub shared: &'a mut SharedBlackboard,
}

pub struct SharedBlackboard {
    pub area_link_cache: HashMap<(WorldArea, WorldArea), bool>,
}

impl ai::Blackboard for AiBlackboard<'_> {
    #[cfg(feature = "metrics")]
    fn entity(&self) -> String {
        use crate::entity_pretty;
        format!("{}", entity_pretty!(self.entity))
    }
}

#[macro_export]
macro_rules! dse {
    ($dse:expr) => {
        AiBox::new($dse) as Box<dyn Dse<AiContext>>
    };
}
