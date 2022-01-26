use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub use action::AiAction;
use common::*;
pub use input::AiInput;
pub use system::{AiComponent, AiSystem};
use unit::world::{WorldPoint, WorldPosition};
use world::WorldArea;

use crate::ai::dse::AdditionalDse;
use crate::ai::input::LocalAreaSearch;

use crate::ai::system::StreamDseData;
use crate::ecs::{EcsWorld, Entity};
use crate::item::{FoundSlot, InventoryComponent, ItemFilter};
use crate::{HungerComponent, SocietyComponent, SocietyHandle, TransformComponent};

mod action;
mod consideration;
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
    type StreamDseExtraData = StreamDseData;
}

/// 'a: only as long as this AI tick
#[derive(Clone)]
pub struct AiBlackboard<'a> {
    pub entity: Entity,
    /// For navigation
    pub accessible_position: WorldPosition,
    pub position: WorldPoint,
    /// None if missing component
    pub hunger: Option<NormalizedFloat>,
    pub inventory: Option<&'a InventoryComponent>,
    pub inventory_search_cache: HashMap<ItemFilter, FoundSlot<'a>>,
    pub society: Option<SocietyHandle>,
    pub ai: &'a AiComponent,

    /// Value is (max distance, results), so smaller ranges can reuse results of bigger ranges
    pub local_area_search_cache: HashMap<ItemFilter, (u32, LocalAreaSearch)>,

    // For fetching other components
    pub world: &'a EcsWorld,
    pub shared: Rc<RefCell<SharedBlackboard>>,
}

#[derive(Default)]
pub struct SharedBlackboard {
    pub area_link_cache: HashMap<(WorldArea, WorldArea), bool>,
}

impl ai::Blackboard for AiBlackboard<'_> {
    #[cfg(feature = "metrics")]
    fn entity(&self) -> String {
        format!("{}", self.entity)
    }
}

#[macro_export]
macro_rules! dse {
    ($dse:expr) => {
        AiBox::new($dse) as Box<dyn Dse<AiContext>>
    };
}

impl<'a> AiBlackboard<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        e: Entity,
        transform: &'a TransformComponent,
        hunger: Option<&'a HungerComponent>,
        inventory: Option<&'a InventoryComponent>,
        society: Option<&'a SocietyComponent>,
        ai: &'a AiComponent,
        shared: Rc<RefCell<SharedBlackboard>>,
        world: &'a EcsWorld,
    ) -> Self {
        AiBlackboard::<'a> {
            entity: e,
            accessible_position: transform.accessible_position(),
            position: transform.position,
            hunger: hunger.map(|h| h.hunger()),
            inventory_search_cache: HashMap::new(),
            local_area_search_cache: HashMap::new(),
            inventory,
            society: society.map(|comp| comp.handle()),
            ai,
            world,
            shared,
        }
    }
}
