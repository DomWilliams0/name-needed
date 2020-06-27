use std::collections::hash_map::Entry;
use std::fmt::{Display, Formatter};

use ai::Context;
use common::*;
use unit::world::WorldPoint;
use world::WorldRef;

use crate::ai::{AiContext, Blackboard, DivineCommandComponent, SharedBlackboard};
use crate::ecs::*;
use crate::item::{BaseItemComponent, ItemFilter, ItemFilterable};
use crate::{InventoryComponent, TransformComponent};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum AiInput {
    /// Hunger level, 0=starving 1=completely full
    Hunger,

    /// Switch, 1=has at least 1 matching filter, 0=none
    HasInInventory(ItemFilter),

    // TODO HasInInventoryGraded - returns number,quality of matches
    CanFindLocally {
        filter: ItemFilter,
        max_radius: u32,
        max_count: u32,
    },

    Constant(OrderedFloat<f32>),

    DivineCommand,
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

            AiInput::DivineCommand => {
                if blackboard
                    .world
                    .component::<DivineCommandComponent>(blackboard.entity)
                    .is_ok()
                {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

fn search_inventory_with_cache(
    blackboard: &mut Blackboard,
    inventory: &InventoryComponent,
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
    blackboard: &mut Blackboard,
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
                blackboard.position,
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
                    blackboard.position,
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
    self_position: WorldPoint,
    world: &EcsWorld,
    shared_bb: &mut SharedBlackboard,
    filter: &ItemFilter,
    max_radius: f32,
    output: &mut LocalAreaSearch,
) {
    // TODO arena allocated vec return value
    // TODO clearly needs some spatial partitioning here

    let (entities, transform, item): (
        Read<EntitiesRes>,
        ReadStorage<TransformComponent>,
        ReadStorage<BaseItemComponent>,
    ) = world.system_data();

    let voxel_world_ref = &*world.read_resource::<WorldRef>();
    let voxel_world = voxel_world_ref.borrow();

    // find the area we are in
    let self_area = match voxel_world.area_for_point(self_position) {
        Some((_, area)) => area,
        None => {
            // we are not in a walkable area, abort
            return;
        }
    };

    let self_position = Vector2::from(self_position);
    let results = (&entities, &transform, &item)
        .join()
        .filter(|(entity, _, item)| {
            // cheap filter check first
            (*entity, *item).matches(*filter).is_some()
        })
        .filter_map(|(entity, transform, item)| {
            // check distance is in range
            let distance = self_position.distance(transform.position.into());
            if distance <= max_radius {
                Some((entity, transform.position, distance, item))
            } else {
                None
            }
        })
        .filter_map(|(entity, point, distance, item)| {
            // check that this item is accessible
            let (_, item_area) = voxel_world.area_for_point(point)?;
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

            if reachable {
                Some((entity, point, distance, item.condition.value()))
            } else {
                None
            }
        });

    output.extend(results);
}

impl Display for AiInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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
            AiInput::DivineCommand => write!(f, "Has divine command"),
        }
    }
}
