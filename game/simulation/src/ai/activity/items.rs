use common::*;

use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish};
use crate::ecs::{ComponentWorld, Entity};
use crate::item::{
    BaseItemComponent, BaseSlotPolicy, InventoryComponent, ItemClass, ItemFilter, ItemReference,
    LooseItemReference, PickupItemComponent, SlotReference, UsingItemComponent,
};
use crate::path::FollowPathComponent;
use crate::TransformComponent;
use common::derive_more::Constructor;
use unit::world::WorldPoint;

pub struct UseHeldItemActivity {
    item: Entity,
    /// Initially set to wherever the item is in inventory, then reset to None in on_start
    initial_slot: Option<SlotReference>,
    exertion: f32,
}

#[derive(Constructor)]
pub struct GoPickUpItemActivity {
    /// See [ItemsToPickUp]
    items: Vec<(Entity, WorldPoint)>,
}

/// Newtype to compare GoPickupItem just by the filter and number of results.
/// Items are in *reverse desirability order* - last is the most desirable, pop that
/// and try the next last if that becomes unavailable
#[derive(Debug, Clone)]
pub struct ItemsToPickUp(pub ItemFilter, pub Vec<(Entity, WorldPoint)>);

#[derive(Debug, Error)]
pub enum PickupError {
    #[error("missing followpath component")]
    NoFollowPathComponent,
}

impl UseHeldItemActivity {
    pub fn new(item: LooseItemReference) -> Self {
        let ItemReference(slot, item) = item.0;
        Self {
            item,
            initial_slot: Some(slot),
            exertion: 0.0,
        }
    }
}

impl<W: ComponentWorld> Activity<W> for UseHeldItemActivity {
    fn on_start(&mut self, ctx: &ActivityContext<W>) {
        // get item info
        let base_item = match ctx.world.component::<BaseItemComponent>(self.item) {
            Ok(base) => base,
            Err(_) => return,
        };

        // TODO proper exertion calculation for item use
        self.exertion = match base_item.class {
            ItemClass::Food => 0.6,
            ItemClass::Weapon => 1.2,
        };

        // TODO equipping will depend on the item's size in base+mounted inventories, not yet implemented
        assert_eq!(base_item.base_slots, 1);
        assert_eq!(base_item.mounted_slots, 1);

        let condition = base_item.condition.value();
        let class = base_item.class;
        let holder = ctx.entity;
        let initial_slot = self.initial_slot.take().unwrap();
        ctx.updates.queue("use held item", move |world| {
            // move item to base inventory if necessary
            let base_slot = match initial_slot {
                SlotReference::Base(slot) => slot,
                mounted_slot @ SlotReference::Mounted(_, _) => {
                    let inventory = world.component_mut::<InventoryComponent>(holder)?;

                    // TODO add ItemUseType which hints at which slot to use
                    let policy = BaseSlotPolicy::AlwaysDominant;

                    inventory.equip(mounted_slot, policy)?
                }
            };

            // start using item
            if let Ok(Some(old)) = world.add_now(
                holder,
                UsingItemComponent {
                    left: condition,
                    base_slot,
                    class,
                },
            ) {
                warn!("overwriting component: {:?}", old);
            };

            Ok(())
        });
    }

    fn on_tick(&mut self, _: &ActivityContext<W>) -> ActivityResult {
        // the system associated with this item does the work
        ActivityResult::Ongoing
    }

    fn on_finish(&mut self, _: Finish, ctx: &ActivityContext<W>) {
        // stop using item
        ctx.world.remove_lazy::<UsingItemComponent>(ctx.entity);
    }

    fn exertion(&self) -> f32 {
        self.exertion
    }
}

impl<W: ComponentWorld> Activity<W> for GoPickUpItemActivity {
    fn on_start(&mut self, ctx: &ActivityContext<W>) {
        if let Some((_, item)) = self.best_item(ctx.world) {
            self.queue_goto_pickup(item, ctx);
        }
    }

    fn on_tick(&mut self, ctx: &ActivityContext<W>) -> ActivityResult {
        if self.items.is_empty() {
            // no more items
            return ActivityResult::Finished(Finish::Succeeded);
        }

        // cache length here because items might be truncated
        let last_index = self.items.len() - 1;
        let new_target = match self.best_item(ctx.world) {
            Some((idx, _)) if idx == last_index => {
                // the last is still the best, keep going
                None
            }
            Some((_, item)) => {
                // a different item is the best now
                Some(item)
            }
            None => {
                // no items left
                return ActivityResult::Finished(Finish::Interrupted);
            }
        };

        if let Some(item) = new_target {
            self.queue_goto_pickup(item, ctx);
        }

        ActivityResult::Ongoing
    }

    fn on_finish(&mut self, _: Finish, ctx: &ActivityContext<W>) {
        ctx.world.remove_lazy::<PickupItemComponent>(ctx.entity);
    }

    fn exertion(&self) -> f32 {
        1.0
    }
}

impl GoPickUpItemActivity {
    fn best_item<W: ComponentWorld>(&mut self, world: &W) -> Option<(usize, (Entity, WorldPoint))> {
        let voxel_ref = world.voxel_world();
        let voxel_world = voxel_ref.borrow();

        // choose the best item that still exists
        let new_best_index = self.items.iter().rposition(|(item, known_pos)| {
            match world
                .component::<TransformComponent>(*item)
                .ok()
                .and_then(|transform| {
                    // still got a transform
                    voxel_world.area_for_point(transform.position)
                }) {
                Some((current_pos, _)) => {
                    // TODO the item moved while going to pick it up, what do
                    assert_eq!(current_pos, known_pos.floor(), "item moved");

                    // this item is good to path find to
                    true
                }
                None => false, // move onto next item because this one is not accessible anymore
            }
        });

        new_best_index.map(|idx| {
            // any items after idx are to be discarded
            self.items.truncate(idx + 1);

            // safety: index returned from rposition
            let item = unsafe { *self.items.get_unchecked(idx) };
            (idx, item)
        })
    }

    fn queue_goto_pickup<W: ComponentWorld>(
        &self,
        item: (Entity, WorldPoint),
        ctx: &ActivityContext<W>,
    ) {
        // path find to the new target
        let holder = ctx.entity;
        let (item, target) = item;
        ctx.updates.queue("path find to item", move |world| {
            let follow = world
                .component_mut::<FollowPathComponent>(holder)
                .map_err(|_| Box::new(PickupError::NoFollowPathComponent))?;

            // TODO dont manually set the exact follow speed - choose a preset e.g. wander,dawdle,walk,fastwalk,run,sprint
            follow.new_path(target, NormalizedFloat::one());

            // attempt to pickup the item when close
            world.add_lazy(holder, PickupItemComponent(item));

            Ok(())
        });
    }
}

impl PartialEq for ItemsToPickUp {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1.len() == other.1.len()
    }
}

impl Eq for ItemsToPickUp {}
