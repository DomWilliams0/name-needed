use crate::helpers::EntityPosition;
use crate::tests::TestHelper;
use crate::{HookContext, HookResult, InitHookResult, TestDeclaration};
use common::*;
use simulation::{
    AiAction, ComponentRefMut, ComponentWorld, ConditionComponent, ContainedInComponent,
    ContainerComponent, Entity, EntityEventDebugPayload, EntityEventPayload,
    EntityLoggingComponent, HungerComponent, InventoryComponent, LoggedEntityEvent,
    PhysicalComponent, TaskResultSummary,
};
use unit::space::{length::Length3, volume::Volume};

pub struct EquipWithPickup<I: ?Sized> {
    human: Entity,
    item: Entity,
    dummy: PhantomData<I>,
}

pub trait InitialInventoryState {
    fn populate(test: &EquipWithPickup<Self>, ctx: &HookContext);

    fn validate(
        test: &EquipWithPickup<Self>,
        ctx: &HookContext,
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

#[derive(Debug)]
enum EventResult {
    Success,
    Failed(String),
}

impl<I: InitialInventoryState> EquipWithPickup<I> {
    fn has_pickup_event(&self, ctx: &HookContext) -> bool {
        let human_picked_up = ctx.events.iter().find_map(|e| {
            if e.subject == self.human {
                if let EntityEventPayload::HasPickedUp(picked_up) = e.payload {
                    if picked_up == self.item {
                        // picked up expected item
                        return Some(EventResult::Success);
                    }
                    return Some(EventResult::Failed("picked up wrong entity".to_owned()));
                }
            }

            None
        });

        let item_was_picked_up = ctx.events.iter().find_map(|e| {
            if e.subject == self.item {
                if let EntityEventPayload::BeenPickedUp(picker_upper, ref result) = e.payload {
                    if picker_upper == self.human && result.is_ok() {
                        // picked up by expected human
                        return Some(EventResult::Success);
                    }

                    return Some(EventResult::Failed(format!(
                        "pickup failed or was by wrong entity: {:?}",
                        e.payload
                    )));
                }
            }

            None
        });

        let mut failures = Vec::new();

        if let Some(EventResult::Failed(ref err)) = human_picked_up {
            failures.push(err.clone());
        }

        if let Some(EventResult::Failed(ref err)) = item_was_picked_up {
            failures.push(err.clone());
        }

        if failures.is_empty() {
            // success if both were triggered
            assert_eq!(
                human_picked_up.is_some(),
                item_was_picked_up.is_some(),
                "both or neither events should have triggered"
            );
            human_picked_up.is_some()
        } else {
            let err = failures.into_iter().join(", ");
            panic!("{}", err)
        }
    }

    fn has_activity_succeeded(&self, ctx: &HookContext) -> bool {
        let result = ctx.events.iter().find_map(|e| {
            if e.subject == self.human {
                if let EntityEventPayload::Debug(EntityEventDebugPayload::FinishedActivity {
                    description,
                    result,
                }) = &e.payload
                {
                    if description.contains("Picking up") {
                        return Some(match result {
                            TaskResultSummary::Succeeded => EventResult::Success,
                            TaskResultSummary::Cancelled => {
                                EventResult::Failed("cancelled".to_owned())
                            }
                            TaskResultSummary::Failed(err) => EventResult::Failed(err.clone()),
                        });
                    }
                }
            }

            None
        });
        match result {
            Some(EventResult::Success) => true,
            Some(other) => panic!("activity failed: {:?}", other),
            None => false,
        }
    }

    fn has_equipped(&self, inv: &InventoryComponent) -> bool {
        inv.has_equipped(self.item)
    }

    pub fn on_tick(&mut self, _test: TestHelper, ctx: &HookContext) -> HookResult {
        if !self.has_activity_succeeded(ctx) {
            return HookResult::KeepGoing;
        }

        let inv = ctx
            .simulation
            .ecs
            .component::<InventoryComponent>(self.human)
            .expect("no inventory");

        I::validate(self, ctx, &*inv)
    }

    pub fn on_init(_test: TestHelper, ctx: &HookContext) -> InitHookResult<Self> {
        let setup = || {
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
            let test = Self {
                human,
                item,
                dummy: PhantomData,
            };

            I::populate(&test, ctx);

            Ok(test)
        };
        setup().into()
    }

    fn inventory<'a>(&self, ctx: &'a HookContext) -> ComponentRefMut<'a, InventoryComponent> {
        ctx.simulation
            .ecs
            .component_mut(self.human)
            .expect("no inventory")
    }
}

// TODO move item creation into dev helpers
impl InitialInventoryState for EmptyInventory {
    fn populate(_: &EquipWithPickup<Self>, _: &HookContext) {
        // leave empty
    }

    fn validate(
        test: &EquipWithPickup<Self>,
        ctx: &HookContext,
        inv: &InventoryComponent,
    ) -> HookResult {
        // should have picked the item up
        if test.has_pickup_event(ctx) {
            assert!(test.has_equipped(inv));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }
}

impl InitialInventoryState for AlreadyEquipped {
    fn populate(test: &EquipWithPickup<Self>, ctx: &HookContext) {
        // add to inventory
        let mut inv = test.inventory(ctx);
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
        ctx: &HookContext,
        inv: &InventoryComponent,
    ) -> HookResult {
        // no pickup event, just equip
        if test.has_equipped(inv) {
            assert!(!test.has_pickup_event(ctx));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }
}

impl InitialInventoryState for AlreadyInInventory {
    fn populate(test: &EquipWithPickup<Self>, ctx: &HookContext) {
        // give container
        let bag = ctx.simulation.ecs.helpers_dev().give_bag(test.human);

        // put item in the container
        let mut container = ctx
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
        ctx: &HookContext,
        inv: &InventoryComponent,
    ) -> HookResult {
        // no pickup event, just equip
        if test.has_equipped(inv) {
            assert!(!test.has_pickup_event(ctx));
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }
}

impl InitialInventoryState for FullInventory {
    fn populate(test: &EquipWithPickup<Self>, ctx: &HookContext) {
        // fill equip slots
        let slot_count = test.inventory(ctx).equip_slots().len();

        for _ in 0..slot_count {
            let item = ctx
                .new_entity("core_food_apple", EntityPosition::Origin)
                .expect("failed to create dummy item");

            let mut inv = test.inventory(ctx);
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
        ctx: &HookContext,
        inv: &InventoryComponent,
    ) -> HookResult {
        // should have picked the item up
        if test.has_pickup_event(ctx) {
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
