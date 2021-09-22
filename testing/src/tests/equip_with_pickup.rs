use crate::helpers::EntityPosition;
use crate::tests::TestHelper;
use crate::{HookContext, HookResult, InitHookResult, TestDeclaration};
use common::*;
use simulation::{
    AiAction, ComponentWorld, ConditionComponent, ContainedInComponent, Entity,
    EntityLoggingComponent, HungerComponent, InventoryComponent, LoggedEntityEvent,
};
use unit::space::{length::Length3, volume::Volume};

pub struct EquipWithPickup<I> {
    human: Entity,
    item: Entity,
    dummy: PhantomData<I>,
}

pub trait InitialInventoryState {
    fn populate(human: Entity, inv: &mut InventoryComponent, ctx: &HookContext);
}

/// Item can be equipped immediately
pub struct EmptyInventory;

/// Item must be manually equipped
pub struct FullInventory;

impl<I: InitialInventoryState> EquipWithPickup<I> {
    pub fn on_tick(&mut self, test: TestHelper, ctx: &HookContext) -> HookResult {
        let logs = ctx
            .simulation
            .ecs
            .component::<EntityLoggingComponent>(self.human)
            .expect("no logging component");

        let found = logs
            .iter_raw_logs()
            .any(|e| *e == LoggedEntityEvent::PickedUp(self.item));
        if found {
            // verify equipped
            let inv = ctx
                .simulation
                .ecs
                .component::<InventoryComponent>(self.human)
                .expect("no inventory");
            assert!(inv.has_equipped(self.item));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }

    pub fn on_init(test: TestHelper, ctx: &HookContext) -> InitHookResult<Self> {
        Self::setup(test, ctx).into()
    }

    fn setup(test: TestHelper, ctx: &HookContext) -> BoxedResult<Self> {
        let human = ctx.new_human(EntityPosition::Origin)?;
        let item = ctx.new_entity("core_food_apple", EntityPosition::Far)?;

        // remove temptation to eat the food
        ctx.simulation.ecs.remove_now::<HungerComponent>(human);

        // go pick it up
        ctx.simulation
            .ecs
            .helpers_dev()
            .force_activity(human, AiAction::GoEquip(item));

        // setup inventory
        let inv = ctx
            .simulation
            .ecs
            .component_mut(human)
            .expect("no inventory");
        I::populate(human, inv, ctx);

        Ok(Self {
            human,
            item,
            dummy: PhantomData,
        })
    }
}

impl InitialInventoryState for EmptyInventory {
    fn populate(_: Entity, _: &mut InventoryComponent, _: &HookContext) {
        // leave empty
    }
}

impl InitialInventoryState for FullInventory {
    fn populate(human: Entity, inv: &mut InventoryComponent, ctx: &HookContext) {
        // fill equip slots
        let slot_count = inv.equip_slots().len();
        for _ in 0..slot_count {
            let item = ctx
                .new_entity("core_food_apple", EntityPosition::Origin)
                .expect("failed to create dummy item");
            let success = inv.insert_item(
                ctx.simulation.ecs,
                item,
                0,
                Volume::new(1),
                Length3::new(1, 1, 1),
                |item, container| {
                    ctx.simulation
                        .ecs
                        .helpers_comps()
                        .add_to_container(item, ContainedInComponent::Container(container));
                },
            );

            ctx.simulation
                .ecs
                .helpers_comps()
                .add_to_container(item, ContainedInComponent::InventoryOf(human));
            assert!(success, "failed to add item to inventory");
        }

        // ensure spare slots
        ctx.simulation.ecs.helpers_dev().give_bag(human);
    }
}

declare_test!(EquipWithPickup<EmptyInventory> EquipWithPickup<FullInventory>);
