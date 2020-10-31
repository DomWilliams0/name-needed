use crate::{ConditionComponent, WorldRef};
use ai::Context;
use common::*;
use unit::world::{WorldPoint, WorldPosition};

use crate::ai::{AiBlackboard, AiContext, SharedBlackboard};
use crate::ecs::*;
use crate::item::{HauledItemComponent, InventoryComponent, ItemFilter, ItemFilterable};
use crate::spatial::Spatial;
use std::collections::hash_map::Entry;
use world::block::BlockType;

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum AiInput {
    /// Hunger level, 0=starving 1=completely full
    Hunger,

    /// Switch, 1=has at least 1 matching filter, 0=none
    HasInInventory(ItemFilter),

    /// Has n extra hands available for hauling the given entity. 1 if already hauling
    HasExtraHandsForHauling(u16, Entity),

    /// Switch, 0=item is unusable e.g. being hauled, 1=usable immediately
    CanUseHeldItem(ItemFilter),

    // TODO HasInInventoryGraded - returns number,quality of matches
    CanFindLocally {
        filter: ItemFilter,
        max_radius: u32,
        max_count: u32,
    },

    Constant(OrderedFloat<f32>),

    /// Distance squared to given pos. Can't be WorldPoint because needs to be Hash
    MyDistance2To(WorldPosition),

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
            AiInput::Hunger => blackboard.hunger.into(),
            AiInput::Constant(c) => c.0,
            AiInput::HasInInventory(filter) => match blackboard.inventory {
                None => 0.0,
                Some(inv) => {
                    if search_inventory_with_cache(blackboard, inv, filter) {
                        1.0
                    } else {
                        0.0
                    }
                }
            },
            AiInput::CanFindLocally {
                filter,
                max_radius,
                max_count,
            } => search_local_area_with_cache(blackboard, filter, *max_radius, *max_count),

            AiInput::MyDistance2To(pos) => {
                let target = pos.centred();
                blackboard.position.distance2(target)
            }
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
                if blackboard
                    .world
                    .component::<HauledItemComponent>(*e)
                    .map(|comp| comp.hauler == blackboard.entity)
                    .unwrap_or(false)
                {
                    // already being hauled by this entity
                    return 1.0;
                }

                let can_haul = blackboard
                    .world
                    .component::<InventoryComponent>(blackboard.entity)
                    .ok()
                    .map(|inv| inv.has_hauling_slots(*n))
                    .unwrap_or(false);

                if can_haul {
                    1.0
                } else {
                    0.0
                }
            }
            AiInput::CanUseHeldItem(filter) => {
                match blackboard.inventory_search_cache.get(filter) {
                    Some(found) => {
                        // resolve found item entity
                        let inventory = blackboard.inventory.expect("item was already found");
                        let item = found.get(inventory, blackboard.world);

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
) -> bool {
    let cache_entry = blackboard.inventory_search_cache.entry(*filter);

    let result = match cache_entry {
        Entry::Vacant(v) => match inventory.search(filter, blackboard.world) {
            Some(item) => Some(*v.insert(item)),
            None => None,
        },
        Entry::Occupied(e) => Some(*e.get()),
    };

    result.is_some()
}

/// (item entity, position, direct distance, item condition)
pub type LocalAreaSearch = Vec<(Entity, WorldPoint, f32, NormalizedFloat)>;

fn search_local_area_with_cache(
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

/// Searches for entities with a `ConditionComponent` only
fn search_local_area(
    self_position: WorldPosition,
    world: &EcsWorld,
    shared_bb: &mut SharedBlackboard,
    filter: &ItemFilter,
    max_radius: f32,
    output: &mut LocalAreaSearch,
) {
    let conditions = world.read_storage::<ConditionComponent>();

    let voxel_world_ref = &*world.read_resource::<WorldRef>();
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

    let spatial = world.resource::<Spatial>();
    let results = spatial
        .query_in_radius(self_position.centred(), max_radius)
        .filter_map(|(entity, pos, dist)| {
            let condition = conditions.get(entity)?;

            // check item filter matches
            (entity, Some(world)).matches(*filter).as_option()?;

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

            reachable.as_some((entity, pos, dist, condition.0.value()))
        });

    output.extend(results);
}

impl Display for AiInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            AiInput::Hunger => write!(f, "Hunger"),
            AiInput::HasInInventory(filter) => write!(f, "Has an item matching {}", filter),
            AiInput::CanFindLocally {
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
                write!(f, "Has {} extra hands for hauling", *n)
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
