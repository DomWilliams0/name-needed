use std::collections::hash_map::Entry;

use ai::Context;
use common::*;
use unit::world::WorldPoint;
use world::block::BlockType;

use crate::ai::{AiBlackboard, AiContext, AiTarget};

use crate::ecs::*;
use crate::item::{
    FoundSlot, HaulableItemComponent, HauledItemComponent, InventoryComponent, ItemFilter,
};

use crate::{ContainedInComponent, TransformComponent};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum AiInput {
    /// Hunger level, 0=starving 1=completely full
    Hunger,

    /// Switch, 1=has at least 1 matching filter, 0=none
    HasInInventory(ItemFilter),

    /// Has n extra hands available for hauling the given entity (or target entity if None).
    /// 1 if already hauling
    HasExtraHandsForHauling(u16, Option<Entity>),

    /// 1.0=has enough hands free to hold/haul/equip target entity
    HasFreeHandsToHoldTarget,

    /// Switch, 0=item is unusable e.g. being hauled, 1=usable immediately
    CanUseHeldItem(ItemFilter),

    // TODO HasInInventoryGraded - returns number,quality of matches
    // TODO should include check for n free slots anywhere in inventory (not just hands)
    /// Does not look in inventory
    CanFindGradedItemsLocally {
        filter: ItemFilter,
        max_radius: u32,
        max_count: u32,
    },

    Constant(OrderedFloat<f32>),

    /// Distance squared to given target
    MyDistance2To(AiTarget),

    /// Distance squared to target entity/position, f32::MAX on error
    MyDistance2ToTarget,

    TargetBlockTypeMatches(BlockTypeMatch),
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum BlockTypeMatch {
    Is(BlockType),
    IsNot(BlockType),
}

impl ai::Input<AiContext> for AiInput {
    fn get(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
        target: Option<&AiTarget>,
    ) -> f32 {
        use AiInput::*;
        match self {
            Hunger => hunger(blackboard),
            HasInInventory(filter) => has_in_inventory(blackboard, filter).unwrap_or(0.0),
            HasExtraHandsForHauling(hands, item) => {
                has_extra_hands_for_hauling(blackboard, *hands, *item, target).unwrap_or(0.0)
            }
            HasFreeHandsToHoldTarget => {
                has_free_hands_to_hold_target(blackboard, target).unwrap_or(0.0)
            }
            CanUseHeldItem(filter) => can_use_held_item(blackboard, filter).unwrap_or(0.0),
            CanFindGradedItemsLocally { .. } => todo!(),
            Constant(f) => f.0,
            MyDistance2To(tgt) => distance_to_target(blackboard, Some(tgt)).unwrap_or(f32::MAX),
            MyDistance2ToTarget => distance_to_target(blackboard, target).unwrap_or(f32::MAX),
            TargetBlockTypeMatches(bt) => {
                target_block_type_matches(blackboard, target, *bt).unwrap_or(0.0)
            }
        }
    }
}

fn hunger(blackboard: &mut AiBlackboard) -> f32 {
    match blackboard.hunger {
        Some(hunger) => hunger.value(),
        None => 1.0, // not hungry if not applicable
    }
}

fn has_in_inventory(blackboard: &mut AiBlackboard, filter: &ItemFilter) -> Option<f32> {
    let inventory = blackboard.inventory?;
    let _found = search_inventory_with_cache(blackboard, inventory, filter)?;
    Some(1.0)
}

fn has_extra_hands_for_hauling(
    blackboard: &mut AiBlackboard,
    extra_hands: u16,
    item: Option<Entity>,
    target: Option<&AiTarget>,
) -> Option<f32> {
    let item = match (item, target) {
        (Some(item), _) => item,
        (None, Some(AiTarget::Entity(item))) => *item,
        _ => {
            warn!("no target found for has_extra_hands_for_hauling input");
            return None;
        }
    };

    let inventory = blackboard.inventory?;
    if inventory.search_equipped(ItemFilter::SpecificEntity(item), Some(blackboard.world)) {
        // already being hauled by this entity
        return Some(1.0);
    }

    Some(if inventory.has_hauling_slots(extra_hands) {
        0.95
    } else {
        0.0
    })
}

fn has_free_hands_to_hold_target(
    blackboard: &mut AiBlackboard,
    target: Option<&AiTarget>,
) -> Option<f32> {
    let target = target.and_then(|t| t.entity())?;
    let inv = blackboard.inventory?;

    let extra_hands = blackboard
        .world
        .component::<HaulableItemComponent>(target)
        .ok()
        .map(|comp| comp.extra_hands)?;

    Some(if inv.has_hauling_slots(extra_hands) {
        1.0
    } else {
        0.0
    })
}

fn can_use_held_item(blackboard: &mut AiBlackboard, filter: &ItemFilter) -> Option<f32> {
    let inventory = blackboard.inventory?;
    let slot = search_inventory_with_cache(blackboard, inventory, filter)?;

    let item = slot.get(inventory, blackboard.world);

    // check if being hauled
    let is_hauled = blackboard.world.has_component::<HauledItemComponent>(item);

    Some(if !is_hauled {
        // fully usable
        1.0
    } else {
        // unusable
        0.0
    })
}

/// 0 distance if in inventory
fn distance_to_target(blackboard: &mut AiBlackboard, target: Option<&AiTarget>) -> Option<f32> {
    let target_pos = match target {
        Some(AiTarget::Entity(e)) => {
            // check if held by us first
            if let Ok(hauled) = blackboard.world.component::<HauledItemComponent>(*e) {
                if hauled.hauler == blackboard.entity {
                    // item is being hauled by us
                    return Some(0.0);
                }
            }

            if let Ok(ContainedInComponent::InventoryOf(holder)) = blackboard
                .world
                .component::<ContainedInComponent>(*e)
                .as_deref()
            {
                if *holder == blackboard.entity {
                    // item is in our inventory
                    return Some(0.0);
                }
            }

            // otherwise use transform
            blackboard
                .world
                .component::<TransformComponent>(*e)
                .ok()
                .map(|pos| pos.position)?
        }
        Some(AiTarget::Point(pos)) => *pos,
        Some(AiTarget::Block(block)) => block.centred(),
        _ => return None,
    };

    Some(target_pos.distance2(blackboard.transform.position))
}

fn target_block_type_matches(
    blackboard: &mut AiBlackboard,
    target: Option<&AiTarget>,
    matches: BlockTypeMatch,
) -> Option<f32> {
    let pos = target.and_then(|t| t.block())?;

    let w = blackboard.world.voxel_world();
    let w = w.borrow();

    let block = w.block(pos)?;
    Some(if matches == block.block_type() {
        1.0
    } else {
        0.0
    })
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

impl Display for AiInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        use AiInput::*;
        match self {
            Hunger => f.write_str("Hunger"),
            HasInInventory(filter) => write!(f, "Has an item matching {}", filter),
            CanFindGradedItemsLocally {
                filter,
                max_radius,
                max_count,
            } => write!(
                f,
                "Can find max {} items in {} radius if {}",
                max_count, max_radius, filter
            ),
            Constant(c) => write!(f, "Constant {:?}", c.0),

            MyDistance2To(pos) => write!(f, "Distance to {}", pos),
            MyDistance2ToTarget => f.write_str("Distance to target"),

            // TODO lowercase BlockType
            TargetBlockTypeMatches(matches) => {
                let (bit, bt) = match matches {
                    BlockTypeMatch::Is(bt) => ("", bt),
                    BlockTypeMatch::IsNot(bt) => ("not ", bt),
                };
                write!(f, "Is target block{} {}", bit, bt)
            }
            HasExtraHandsForHauling(n, _e) => {
                write!(f, "Has {} extra hands for hauling", n)
            }
            CanUseHeldItem(filter) => write!(f, "Can use held item matching {}", filter),
            HasFreeHandsToHoldTarget => f.write_str("Has free hands to hold target entity"),
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
