use crate::{ConditionComponent, ContainedInComponent, SocietyComponent, SocietyHandle, WorldRef};
use ai::Context;
use common::*;
use unit::world::{WorldPoint, WorldPosition};

use crate::ai::{AiBlackboard, AiContext, SharedBlackboard};
use crate::build::ReservedMaterialComponent;
use crate::ecs::*;
use crate::item::{FoundSlot, HauledItemComponent, InventoryComponent, ItemFilter, ItemFilterable};
use crate::spatial::{Spatial, Transforms};
use std::collections::hash_map::Entry;
use world::block::BlockType;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum AiInput {
    /// Hunger level, 0=starving 1=completely full
    Hunger,

    /// Switch, 1=has at least 1 matching filter, 0=none
    HasInInventory(ItemFilter),

    /// Has n extra hands available for hauling the given entity. 1 if already hauling one matching
    /// the filter
    HasExtraHandsForHauling(u16, Option<ItemFilter>),

    /// Switch, 0=item is unusable e.g. being hauled, 1=usable immediately
    CanUseHeldItem(ItemFilter),

    // TODO HasInInventoryGraded - returns number,quality of matches
    // TODO should include check for n free slots anywhere in inventory (not just hands)
    CanFindGradedItems {
        filter: ItemFilter,
        max_radius: u32,
        max_count: u32,
    },

    Constant(OrderedFloat<f32>),

    /// Distance squared to given pos
    MyDistance2To(WorldPoint),

    BlockTypeMatches(WorldPosition, BlockTypeMatch),
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum BlockTypeMatch {
    Is(BlockType),
    IsNot(BlockType),
}

impl ai::Input<AiContext> for AiInput {
    fn get(&self, blackboard: &mut <AiContext as Context>::Blackboard) -> f32 {
        match self {
            // "full up" if not applicable
            AiInput::Hunger => blackboard.hunger.map(Into::into).unwrap_or(1.0),
            AiInput::Constant(c) => c.0,
            AiInput::HasInInventory(filter) => match blackboard.inventory {
                None => 0.0,
                Some(inv) => {
                    if search_inventory_with_cache(blackboard, inv, filter).is_some() {
                        1.0
                    } else {
                        0.0
                    }
                }
            },
            AiInput::CanFindGradedItems {
                filter,
                max_radius,
                max_count,
            } => {
                if let Some(inv) = blackboard.inventory {
                    if search_inventory_with_cache(blackboard, inv, filter).is_some() {
                        // found in inventory
                        trace!("matching item found in inventory"; "filter" => ?filter);
                        return 1.0;
                    }
                }
                search_local_area_with_cache_graded(blackboard, filter, *max_radius, *max_count)
            }

            AiInput::MyDistance2To(pos) => blackboard.position.distance2(*pos),
            AiInput::BlockTypeMatches(pos, bt_match) => {
                let world = blackboard.world.voxel_world();
                let block_type = world
                    .borrow()
                    .block(*pos)
                    .map(|b| b.block_type())
                    .unwrap_or(BlockType::Air);
                if *bt_match == block_type {
                    1.0
                } else {
                    0.0
                }
            }
            AiInput::HasExtraHandsForHauling(n, e) => {
                if let Some(filter) = e {
                    if let Some(inventory) = blackboard.inventory {
                        if inventory.search_equipped(*filter, Some(blackboard.world)) {
                            // already being hauled by this entity
                            return 1.0;
                        }
                    }
                }

                let can_haul = blackboard
                    .inventory
                    .map(|inv| inv.has_hauling_slots(*n))
                    .unwrap_or(false);

                if can_haul {
                    1.0
                } else {
                    0.0
                }
            }
            AiInput::CanUseHeldItem(filter) => {
                match blackboard.inventory.and_then(|inv| {
                    search_inventory_with_cache(blackboard, inv, filter).map(|slot| (inv, slot))
                }) {
                    Some((inventory, slot)) => {
                        let item = slot.get(inventory, blackboard.world);

                        // check if being hauled
                        let is_hauled = blackboard.world.has_component::<HauledItemComponent>(item);

                        if !is_hauled {
                            // fully usable
                            1.0
                        } else {
                            // unusable
                            0.0
                        }
                    }
                    None => 0.0,
                }
            }
        }
    }
}

fn search_inventory_with_cache<'a>(
    blackboard: &mut AiBlackboard<'a>,
    inventory: &'a InventoryComponent,
    filter: &ItemFilter,
) -> Option<FoundSlot<'a>> {
    let cache_entry = blackboard.inventory_search_cache.entry(*filter);

    match cache_entry {
        Entry::Vacant(v) => inventory
            .search(filter, blackboard.world)
            .map(|item| *v.insert(item)),
        Entry::Occupied(e) => Some(*e.get()),
    }
}

/// (item entity, position, direct distance, item condition)
pub type LocalAreaSearch = Vec<(Entity, WorldPoint, f32, NormalizedFloat)>;

fn search_local_area_with_cache_graded(
    blackboard: &mut AiBlackboard,
    filter: &ItemFilter,
    max_radius: u32,
    max_count: u32,
) -> f32 {
    let cache_entry = blackboard.local_area_search_cache.entry(*filter);

    let max_radius_float = max_radius as f32;
    let search = match cache_entry {
        Entry::Vacant(v) => {
            let mut results = Vec::new();
            search_local_area(
                blackboard.society,
                blackboard.accessible_position,
                blackboard.world,
                blackboard.shared,
                filter,
                max_radius_float,
                &mut results,
            );

            let (_, search) = v.insert((max_radius, results));
            search as &LocalAreaSearch
        }

        Entry::Occupied(e) => {
            let (cached_range, _) = e.get();
            if max_radius <= *cached_range {
                // use the bigger range and filter
                &e.into_mut().1
            } else {
                // current range > cached range, do the search again and replace the smaller values
                let (range, results_mut) = e.into_mut();

                // reuse search buffer
                // TODO old results are a subset of new results, should reuse
                results_mut.clear();
                search_local_area(
                    blackboard.society,
                    blackboard.accessible_position,
                    blackboard.world,
                    blackboard.shared,
                    filter,
                    max_radius_float,
                    results_mut,
                );
                *range = max_radius;

                results_mut as &LocalAreaSearch
            }
        }
    };

    trace!("found {count} local items", count = search.len(); "filter" => ?filter);
    if search.is_empty() {
        0.0
    } else {
        search
            .iter()
            .take(max_count as usize)
            .map(|(e, _, dist, c)| {
                // scale distance to the max radius provided - closest=1, furthest=0
                let scaled_dist = Proportion::with_value(*dist as u32, max_radius);
                (e, 1.0 - scaled_dist.proportion(), c)
            })
            .map(|(_, closeness, condition)| {
                // sum closeness*condition, so good close items rate highest
                closeness * condition.value()
            })
            .sum()
    }
}

fn search_local_area(
    my_society: Option<SocietyHandle>,
    self_position: WorldPosition,
    world: &EcsWorld,
    shared_bb: &mut SharedBlackboard,
    filter: &ItemFilter,
    max_radius: f32,
    output: &mut LocalAreaSearch,
) {
    let voxel_world_ref = &*world.resource::<WorldRef>();
    let voxel_world = voxel_world_ref.borrow();

    // find the area we are in
    let self_area = match voxel_world.area(self_position).ok() {
        Some(area) => area,
        None => {
            // we are not in a walkable area, abort
            trace!("position is not walkable"; "position" => %self_position);
            return;
        }
    };

    let conditions = world.read_storage::<ConditionComponent>();
    let reservations = world.read_storage::<ReservedMaterialComponent>();
    let containeds = world.read_storage::<ContainedInComponent>();

    let spatial = world.resource::<Spatial>();
    let transforms = Transforms::from(world);
    let results = spatial
        .query_in_radius(transforms, self_position.centred(), max_radius)
        .filter_map(|(entity, pos, dist)| {
            // check item filter matches
            if !(entity, Some(world)).matches(*filter) {
                return None;
            }

            // ensure the item is not held by someone
            if entity.has(&containeds) {
                return None;
            }

            // check if this item is reserved by our society
            if let Some(my_society) = my_society {
                if let Some(reserved) = reservations.get(entity.into()) {
                    if reserved.build_job.society() == my_society {
                        // dont bother considering
                        return None;
                    }
                }
            }

            // check this item is accessible
            // TODO use accessible position?
            let item_area = voxel_world.area(pos.floor()).ok()?;
            let mut reachable;

            // same area, definitely accessible
            reachable = item_area == self_area;

            if !reachable {
                // different areas, do a cached cheap path find to see if its accessible
                // consistent key ordering
                let cache_key = if self_area < item_area {
                    (self_area, item_area)
                } else {
                    (item_area, self_area)
                };
                reachable = *shared_bb
                    .area_link_cache
                    .entry(cache_key)
                    .or_insert_with(|| voxel_world.area_path_exists(self_area, item_area));
            }

            let condition = entity
                .get(&conditions)
                .map(|comp| comp.0.value())
                .unwrap_or_else(NormalizedFloat::one);

            reachable.as_some((entity, pos, dist, condition))
        });

    output.extend(results);
}

impl Display for AiInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            AiInput::Hunger => write!(f, "Hunger"),
            AiInput::HasInInventory(filter) => write!(f, "Has an item matching {}", filter),
            AiInput::CanFindGradedItems {
                filter,
                max_radius,
                max_count,
            } => write!(
                f,
                "Can find max {} items in {} radius if {}",
                max_count, max_radius, filter
            ),
            AiInput::Constant(_) => write!(f, "Constant"),

            AiInput::MyDistance2To(pos) => write!(f, "Distance to {}", pos),

            // TODO lowercase BlockType
            AiInput::BlockTypeMatches(pos, bt_match) => write!(f, "{} at {}", bt_match, pos),
            AiInput::HasExtraHandsForHauling(n, _) => {
                write!(f, "Has {} extra hands for hauling", n)
            }
            AiInput::CanUseHeldItem(filter) => write!(f, "Can use held item matching {}", filter),
        }
    }
}

impl Display for BlockTypeMatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            BlockTypeMatch::Is(bt) => write!(f, "Is block {}", bt),
            BlockTypeMatch::IsNot(bt) => write!(f, "Is block not {}", bt),
        }
    }
}

impl PartialEq<BlockType> for BlockTypeMatch {
    fn eq(&self, other: &BlockType) -> bool {
        match self {
            BlockTypeMatch::Is(bt) => bt == other,
            BlockTypeMatch::IsNot(bt) => bt != other,
        }
    }
}
