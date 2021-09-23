use crate::helpers::EntityPosition;
use crate::tests::TestHelper;
use crate::{HookContext, HookResult, InitHookResult, TestDeclaration};
use common::*;
use simulation::{
    AiAction, ComponentWorld, ConditionComponent, ContainedInComponent, ContainerComponent, Entity,
    EntityLoggingComponent, HungerComponent, InventoryComponent, LoggedEntityEvent,
    PhysicalComponent,
};
use unit::space::{length::Length3, volume::Volume};

pub struct EquipWithPickup<I: ?Sized> {
    human: Entity,
    item: Entity,
    dummy: PhantomData<I>,
}

pub trait InitialInventoryState {
    fn populate(test: &EquipWithPickup<Self>, inv: &mut InventoryComponent, ctx: &HookContext);

    fn validate(
        test: &EquipWithPickup<Self>,
        logs: &EntityLoggingComponent,
        inv: &InventoryComponent,
    ) -> HookResult;
}

/// Item can be equipped immediately
pub struct EmptyInventory;

/// Item must be manually equipped
pub struct FullInventory;

/// Item is already in equip slot
pub struct AlreadyEquipped;

/// Item is already in a held container
pub struct AlreadyInInventory;

impl<I: InitialInventoryState> EquipWithPickup<I> {
    // TODO actually subscribe to the entity event to get Ok/Err, instead of just success like this
    fn has_pickup_event(&self, logging: &EntityLoggingComponent) -> bool {
        logging
            .iter_raw_logs()
            .any(|e| *e == LoggedEntityEvent::PickedUp(self.item))
    }

    fn has_equipped(&self, inv: &InventoryComponent) -> bool {
        inv.has_equipped(self.item)
    }

    pub fn on_tick(&mut self, test: TestHelper, ctx: &HookContext) -> HookResult {
        let logs = ctx
            .simulation
            .ecs
            .component::<EntityLoggingComponent>(self.human)
            .expect("no logging component");

        let inv = ctx
            .simulation
            .ecs
            .component::<InventoryComponent>(self.human)
            .expect("no inventory");

        I::validate(self, logs, inv)
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

        let test = Self {
            human,
            item,
            dummy: PhantomData,
        };

        I::populate(&test, inv, ctx);

        Ok(test)
    }
}

impl InitialInventoryState for EmptyInventory {
    fn populate(_: &EquipWithPickup<Self>, _: &mut InventoryComponent, _: &HookContext) {
        // leave empty
    }

    fn validate(
        test: &EquipWithPickup<Self>,
        logs: &EntityLoggingComponent,
        inv: &InventoryComponent,
    ) -> HookResult {
        // should have picked the item up
        if test.has_pickup_event(logs) {
            assert!(test.has_equipped(inv));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }
}

impl InitialInventoryState for AlreadyEquipped {
    fn populate(test: &EquipWithPickup<Self>, inv: &mut InventoryComponent, ctx: &HookContext) {
        // add to inventory
        assert!(
            inv.insert_item(
                ctx.simulation.ecs,
                test.item,
                0,
                Volume::new(1),
                Length3::new(1, 1, 1),
                |item, container| {
                    ctx.simulation
                        .ecs
                        .helpers_comps()
                        .add_to_container(item, ContainedInComponent::Container(container));
                },
            ),
            "failed to add item to inventory"
        );

        ctx.simulation
            .ecs
            .helpers_comps()
            .add_to_container(test.item, ContainedInComponent::InventoryOf(test.human));
    }

    fn validate(
        test: &EquipWithPickup<Self>,
        logs: &EntityLoggingComponent,
        inv: &InventoryComponent,
    ) -> HookResult {
        // no pickup event, just equip
        if test.has_equipped(inv) {
            assert!(!test.has_pickup_event(logs));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }
}

impl InitialInventoryState for AlreadyInInventory {
    fn populate(test: &EquipWithPickup<Self>, inv: &mut InventoryComponent, ctx: &HookContext) {
        // give container
        let bag = ctx.simulation.ecs.helpers_dev().give_bag(test.human);

        // put item in the container
        let container = ctx
            .simulation
            .ecs
            .component_mut::<ContainerComponent>(bag)
            .expect("no container on bag");
        let phys = ctx
            .simulation
            .ecs
            .component::<PhysicalComponent>(test.item)
            .expect("no physical on item");
        container
            .container
            .add_with(test.item, phys.volume, phys.size)
            .expect("failed to add item to bag");

        // update components
        ctx.simulation
            .ecs
            .helpers_comps()
            .add_to_container(test.item, ContainedInComponent::Container(bag));
    }

    fn validate(
        test: &EquipWithPickup<Self>,
        logs: &EntityLoggingComponent,
        inv: &InventoryComponent,
    ) -> HookResult {
        // no pickup event, just equip
        if test.has_equipped(inv) {
            assert!(!test.has_pickup_event(logs));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }
}

impl InitialInventoryState for FullInventory {
    fn populate(test: &EquipWithPickup<Self>, inv: &mut InventoryComponent, ctx: &HookContext) {
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
                .add_to_container(item, ContainedInComponent::InventoryOf(test.human));
            assert!(success, "failed to add item to inventory");
        }

        // ensure spare slots
        ctx.simulation.ecs.helpers_dev().give_bag(test.human);
    }

    fn validate(
        test: &EquipWithPickup<Self>,
        logs: &EntityLoggingComponent,
        inv: &InventoryComponent,
    ) -> HookResult {
        // should have picked the item up
        if test.has_pickup_event(logs) {
            assert!(test.has_equipped(inv));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }
}

declare_test!(
    EquipWithPickup<EmptyInventory>
    EquipWithPickup<AlreadyEquipped>
    EquipWithPickup<AlreadyInInventory>
    EquipWithPickup<FullInventory>
);
