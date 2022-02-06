use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use common::*;
use unit::world::{WorldPoint, WorldPosition};
use world::WorldArea;

use crate::ai::dse::AdditionalDse;
use crate::ai::input::LocalAreaSearch;
use crate::ai::system::StreamDseData;
use crate::ai::AiInput;
use crate::build::ReservedMaterialComponent;
use crate::ecs::*;
use crate::item::{FoundSlot, ItemFilter, ItemFilterable};
use crate::spatial::Spatial;
use crate::{
    AiAction, ContainedInComponent, EcsWorld, Entity, HungerComponent, InventoryComponent,
    SocietyComponent, SocietyHandle, TransformComponent, WorldRef,
};

pub struct AiContext;

impl ai::Context for AiContext {
    /// TODO ideally this would use ai::Context<'a> to represent the AI tick lifetime: https://github.com/rust-lang/rust/issues/44265
    type Blackboard = AiBlackboard<'static>;
    type Input = AiInput;
    type Action = AiAction;
    type AdditionalDseId = AdditionalDse;
    type StreamDseExtraData = StreamDseData;
    type DseTarget = AiTarget;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub enum AiTarget {
    Entity(Entity),
    Block(WorldPosition),
    Point(WorldPoint),
}

/// 'a: only as long as this AI tick
#[derive(Clone)]
pub struct AiBlackboard<'a> {
    pub entity: Entity,
    pub transform: &'a TransformComponent,
    /// None if missing component
    pub hunger: Option<NormalizedFloat>,
    pub inventory: Option<&'a InventoryComponent>,
    pub inventory_search_cache: HashMap<ItemFilter, FoundSlot<'a>>,
    pub society: Option<SocietyHandle>,

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
    fn entity(&self) -> std::borrow::Cow<str> {
        use crate::alloc::FrameAllocator;
        use std::fmt::Write;

        let alloc = self.world.resource::<FrameAllocator>();
        let mut s = bumpalo::collections::String::new_in(alloc.allocator());
        let _ = write!(&mut s, "{}", self.entity);
        std::borrow::Cow::Borrowed(s.into_bump_str())
    }
}

#[macro_export]
macro_rules! dse {
    ($dse:expr) => {
        AiBox::new($dse) as Box<dyn Dse<AiContext>>
    };
}

pub struct FoundItem {
    pub entity: Entity,
    pub position: WorldPoint,
    pub distance: f32,
}

impl<'a> AiBlackboard<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        e: Entity,
        transform: &'a TransformComponent,
        hunger: Option<&'a HungerComponent>,
        inventory: Option<&'a InventoryComponent>,
        society: Option<&'a SocietyComponent>,
        shared: Rc<RefCell<SharedBlackboard>>,
        world: &'a EcsWorld,
    ) -> Self {
        AiBlackboard::<'a> {
            entity: e,
            transform,
            hunger: hunger.map(|h| h.hunger()),
            inventory_search_cache: HashMap::new(),
            local_area_search_cache: HashMap::new(),
            inventory,
            society: society.map(|comp| comp.handle()),
            world,
            shared,
        }
    }

    /// Searches area in radius for entities that are:
    ///     * accessible from current position
    ///     * aren't held by ANYONE ELSE
    ///     * aren't stored in a container
    ///     * not reserved by this entity's society
    ///
    /// Stops early if `limit` successful are found, or callback returns false
    // TODO cache searches as before in 2f1fc7a if necessary (profile!)
    pub fn search_local_entities(
        &self,
        filter: ItemFilter,
        max_radius: f32,
        limit: usize,
        mut found: impl FnMut(FoundItem) -> bool,
    ) {
        let world = self.world;
        let voxel_world_ref = &*world.resource::<WorldRef>();
        let voxel_world = voxel_world_ref.borrow();
        let self_position = self.transform.accessible_position();

        // find the area we are in
        let self_area = match voxel_world.area(self_position).ok() {
            Some(area) => area,
            None => {
                // we are not in a walkable area, abort
                trace!("position is not walkable"; "position" => %self_position);
                return;
            }
        };

        let reservations = world.read_storage::<ReservedMaterialComponent>();
        let containeds = world.read_storage::<ContainedInComponent>();
        let transforms = world.read_storage::<TransformComponent>();

        let spatial = world.resource::<Spatial>();
        let results = spatial
            .query_in_radius(world, self.transform.position, max_radius)
            .filter_map(|(entity, pos, dist)| {
                // check item filter matches
                if !(entity, Some(world)).matches(filter) {
                    return None;
                }

                // ensure the item is not held by someone else
                match entity.get(&containeds) {
                    Some(ContainedInComponent::InventoryOf(holder)) if *holder == self.entity => {
                        // held by entity, include
                    }
                    Some(comp) if comp.is_in_world() => {
                        // available in the world e.g. item stack
                    }
                    None => {
                        // fine
                    }
                    _ => return None,
                }

                // check if this item is reserved by our society
                if let Some(my_society) = self.society {
                    if let Some(reserved) = reservations.get(entity.into()) {
                        if reserved.build_job.society() == my_society {
                            // dont bother considering
                            return None;
                        }
                    }
                }

                // check this item is accessible
                let item_pos = entity
                    .get(&transforms)
                    .and_then(|t| t.accessible_position)?;
                let item_area = voxel_world.area(item_pos).ok()?;

                if item_area != self_area {
                    // different areas, do a cached cheap path find to see if its accessible
                    // consistent key ordering
                    let cache_key = if self_area < item_area {
                        (self_area, item_area)
                    } else {
                        (item_area, self_area)
                    };
                    if !*self
                        .shared
                        .borrow_mut()
                        .area_link_cache
                        .entry(cache_key)
                        .or_insert_with(|| voxel_world.area_path_exists(self_area, item_area))
                    {
                        return None;
                    }
                }

                // item is accessible
                if found(FoundItem {
                    entity,
                    position: pos,
                    distance: dist,
                }) {
                    Some(())
                } else {
                    None
                }
            })
            .take(limit);

        // consume iterator
        for _ in results {}
    }
}

impl AiTarget {
    #[inline(never)]
    fn expected<T>(&self, what: &'static str) -> Option<T> {
        warn!(
            "unexpected ai target type, expected '{}' but got '{:?}'",
            what, self
        );
        None
    }
    pub fn entity(&self) -> Option<Entity> {
        match self {
            AiTarget::Entity(e) => Some(*e),
            _ => self.expected("entity"),
        }
    }

    pub fn block(&self) -> Option<WorldPosition> {
        match self {
            AiTarget::Block(pos) => Some(*pos),
            _ => self.expected("block"),
        }
    }

    pub fn point(&self) -> Option<WorldPoint> {
        match self {
            AiTarget::Point(pos) => Some(*pos),
            _ => self.expected("point"),
        }
    }
}

impl Display for AiTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            AiTarget::Entity(e) => e as &dyn Display,
            AiTarget::Block(p) => p as &dyn Display,
            AiTarget::Point(p) => p as &dyn Display,
        };
        Display::fmt(display, f)
    }
}
