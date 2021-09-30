use crate::helpers::EntityPosition;
use crate::tests::haul::helpers::*;
use crate::tests::TestHelper;
use crate::{HookContext, HookResult, InitHookResult, TestDeclaration};
use common::BoxedResult;
use simulation::{
    validate_all_inventories, AiAction, BlockType, ComponentWorld, ContainerComponent, EcsWorld,
    Entity, EntityEventDebugPayload, EntityEventPayload, HaulTarget, QueuedUpdates,
    TaskResultSummary, TerrainUpdatesRes, TransformComponent, WorldPosition, WorldPositionRange,
    WorldTerrainUpdate,
};
use std::cell::RefCell;
use std::rc::Rc;

pub struct Haul<H> {
    hauler: Entity,
    item: Entity,
    /// Populated on second game tick
    src_tgt: Rc<RefCell<Option<(HaulTarget, HaulTarget)>>>,
    variant: Rc<RefCell<H>>,
}

pub trait HaulVariant: Sized + 'static {
    fn init(ctx: &HookContext) -> BoxedResult<Self>;

    /// Run as queued update after 1st game tick to allow containers to be initialised
    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)>;

    fn validate_tick(
        &mut self,
        haul: &Haul<Self>,
        ctx: &HookContext,
        test: TestHelper,
    ) -> HookResult {
        match haul.has_activity_succeeded(ctx) {
            Some(EventResult::Success) => {}
            Some(EventResult::Failed(err)) => return HookResult::TestFailure(err),
            None => return HookResult::KeepGoing,
        }

        assert!(haul.has_haul_event(ctx), "item missing haul event");

        let (src, tgt) = haul.src_tgt.borrow().expect("haul targets not populated");

        match tgt {
            HaulTarget::Position(expected_pos) => {
                let pos = ctx
                    .simulation
                    .ecs
                    .component::<TransformComponent>(haul.item)
                    .expect("item has no transform");
                assert!(
                    pos.position.is_almost(&expected_pos.centred(), 1.0),
                    "item is not at destination haul position"
                );
            }
            HaulTarget::Container(e) => {
                let container = ctx
                    .simulation
                    .ecs
                    .component::<ContainerComponent>(e)
                    .expect("cant find destination container");

                assert!(
                    container.container.contains(haul.item),
                    "item is not in destination container"
                );
            }
        }

        if let HaulTarget::Container(e) = src {
            let container = ctx
                .simulation
                .ecs
                .component::<ContainerComponent>(e)
                .expect("cant find source container");

            assert!(
                !container.container.contains(haul.item),
                "item is still in source container"
            );
        }

        HookResult::TestSuccess
    }
}

/// Haul between containers
#[derive(Copy, Clone)]
pub struct ContainerToContainer {
    chest_a: WorldPosition,
    chest_b: WorldPosition,
}

/// Haul from a container to a position
#[derive(Copy, Clone)]
pub struct ContainerToPosition {
    chest_a: WorldPosition,
    target: WorldPosition,
}

/// Haul from a position to a container
#[derive(Copy, Clone)]
pub struct PositionToContainer {
    source: WorldPosition,
    chest_b: WorldPosition,
}

/// Haul between positions
#[derive(Copy, Clone)]
pub struct PositionToPosition {
    source: WorldPosition,
    target: WorldPosition,
}

/// Haul between positions but the item is killed during the haul
pub struct RemoveItemDuring {
    nested: PositionToPosition,
    killed_item: bool,
}

/// Haul between containers but the container is killed before delivery
pub struct RemoveTargetContainerDuring {
    nested: ContainerToContainer,
    killed_container: bool,
}

/// Haul between containers but the container is killed before pickup
pub struct RemoveSourceContainerDuring {
    nested: ContainerToContainer,
    killed_container: bool,
}

// TODO share between tests
#[derive(Debug)]
enum EventResult {
    Success,
    Failed(String),
}

impl<H: HaulVariant + 'static> Haul<H> {
    // TODO move this to a helper, mostly duplicated in equip tests
    fn has_activity_succeeded(&self, ctx: &HookContext) -> Option<EventResult> {
        ctx.events.iter().find_map(|e| {
            if e.subject == self.hauler {
                if let EntityEventPayload::Debug(EntityEventDebugPayload::FinishedActivity {
                    description,
                    result,
                }) = &e.payload
                {
                    if description.contains("Hauling") {
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
        })
    }

    fn has_haul_event(&self, ctx: &HookContext) -> bool {
        let item_was_hauled = ctx.events.iter().find_map(|e| {
            if e.subject == self.item {
                if let EntityEventPayload::Hauled(hauler, ref result) = e.payload {
                    if hauler == self.hauler && result.is_ok() {
                        // hauled by expected human
                        return Some(EventResult::Success);
                    }

                    return Some(EventResult::Failed(format!(
                        "haul failed or was by wrong entity: {:?}",
                        e.payload
                    )));
                }
            }

            None
        });

        if let Some(EventResult::Failed(ref err)) = item_was_hauled {
            panic!("{}", err)
        }

        item_was_hauled.is_some()
    }

    pub fn on_tick(&mut self, test: TestHelper, ctx: &HookContext) -> HookResult {
        let mut variant = self.variant.borrow_mut();
        validate_all_inventories(ctx.simulation.ecs);
        variant.validate_tick(self, ctx, test)
    }

    pub fn on_init(test: TestHelper, ctx: &HookContext) -> InitHookResult<Self> {
        let setup = || -> BoxedResult<Self> {
            let human = ctx.new_human(EntityPosition::Origin)?;
            let item = ctx.new_entity("core_brick_stone", EntityPosition::Far)?;

            let variant = Rc::new(RefCell::new(H::init(ctx)?));
            let variant_clone = variant.clone();

            let src_tgt = Rc::new(RefCell::new(None));
            let src_tgt_clone = src_tgt.clone();

            // need to wait a tick for containers to be created
            ctx.simulation.ecs.resource::<QueuedUpdates>().queue(
                "start haul behaviour for test",
                move |mut world| {
                    let mut do_it = || -> BoxedResult<()> {
                        let mut variant = variant_clone.borrow_mut();
                        let (src, tgt) = variant.src_tgt(&mut *world)?;

                        *src_tgt_clone.borrow_mut() = Some((src, tgt));

                        // put item in src location
                        match src {
                            HaulTarget::Position(pos) => {
                                let transform = world.component_mut::<TransformComponent>(item)?;
                                transform.reset_position(pos.centred());
                            }
                            HaulTarget::Container(c) => {
                                world.helpers_dev().put_item_into_container(item, c);
                            }
                        }

                        // force haul
                        world
                            .helpers_dev()
                            .force_activity(human, AiAction::Haul(item, src, tgt));

                        Ok(())
                    };

                    do_it().expect("queued update failed");
                    Ok(())
                },
            );

            Ok(Self {
                hauler: human,
                item,
                src_tgt,
                variant,
            })
        };

        setup().into()
    }
}

mod helpers {
    use super::*;
    use simulation::AssociatedBlockData;

    fn create_chest(ctx: &HookContext, pos: WorldPosition) -> WorldPosition {
        let terrain_updates = ctx.simulation.ecs.resource_mut::<TerrainUpdatesRes>();
        terrain_updates.push(WorldTerrainUpdate::new(
            WorldPositionRange::with_single(pos),
            BlockType::Chest,
        ));
        pos
    }

    /// Panics if not already a chest
    pub fn destroy_chest(ctx: &HookContext, pos: WorldPosition) {
        assert!(
            resolve_chest(ctx.simulation.ecs, pos).is_ok(),
            "no container to destroy at {}",
            pos
        );

        let terrain_updates = ctx.simulation.ecs.resource_mut::<TerrainUpdatesRes>();
        terrain_updates.push(WorldTerrainUpdate::new(
            WorldPositionRange::with_single(pos),
            BlockType::Air,
        ));
    }

    pub fn create_chest_1(ctx: &HookContext) -> WorldPosition {
        create_chest(ctx, src_pos())
    }

    pub fn create_chest_2(ctx: &HookContext) -> WorldPosition {
        create_chest(ctx, tgt_pos())
    }

    pub fn src_pos() -> WorldPosition {
        (4, 2, 1).into()
    }

    pub fn tgt_pos() -> WorldPosition {
        (1, 12, 1).into()
    }

    pub fn resolve_chest(world: &EcsWorld, container: WorldPosition) -> BoxedResult<Entity> {
        let pos = container;
        let w = world.voxel_world();
        let w = w.borrow();
        if let Some(AssociatedBlockData::Container(container)) = w.associated_block_data(pos) {
            Ok(*container)
        } else {
            Err(format!("container entity not found at {}", container).into())
        }
    }
}

impl HaulVariant for ContainerToContainer {
    fn init(ctx: &HookContext) -> BoxedResult<Self> {
        let a = create_chest_1(ctx);
        let b = create_chest_2(ctx);

        Ok(Self {
            chest_a: a,
            chest_b: b,
        })
    }

    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)> {
        let a = resolve_chest(world, self.chest_a)?;
        let b = resolve_chest(world, self.chest_b)?;

        Ok((HaulTarget::Container(a), HaulTarget::Container(b)))
    }
}

impl HaulVariant for ContainerToPosition {
    fn init(ctx: &HookContext) -> BoxedResult<Self> {
        let a = create_chest_1(ctx);
        let b = tgt_pos();

        Ok(Self {
            chest_a: a,
            target: b,
        })
    }

    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)> {
        let a = resolve_chest(world, self.chest_a)?;
        Ok((HaulTarget::Container(a), HaulTarget::Position(self.target)))
    }
}

impl HaulVariant for PositionToContainer {
    fn init(ctx: &HookContext) -> BoxedResult<Self> {
        let a = src_pos();
        let b = create_chest_2(ctx);

        Ok(Self {
            source: a,
            chest_b: b,
        })
    }

    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)> {
        let b = resolve_chest(world, self.chest_b)?;
        Ok((HaulTarget::Position(self.source), HaulTarget::Container(b)))
    }
}

impl HaulVariant for PositionToPosition {
    fn init(ctx: &HookContext) -> BoxedResult<Self> {
        let a = src_pos();
        let b = tgt_pos();

        Ok(Self {
            source: a,
            target: b,
        })
    }

    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)> {
        Ok((
            HaulTarget::Position(self.source),
            HaulTarget::Position(self.target),
        ))
    }
}

impl HaulVariant for RemoveItemDuring {
    fn init(ctx: &HookContext) -> BoxedResult<Self> {
        PositionToPosition::init(ctx).map(|nested| Self {
            nested,
            killed_item: false,
        })
    }

    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)> {
        self.nested.src_tgt(world)
    }

    fn validate_tick(
        &mut self,
        haul: &Haul<Self>,
        ctx: &HookContext,
        test: TestHelper,
    ) -> HookResult {
        match haul.has_activity_succeeded(ctx) {
            Some(EventResult::Success) => panic!("haul activity shouldn't have succeeded"),
            Some(EventResult::Failed(failure)) => {
                assert!(self.killed_item, "failed before item was killed");
                assert!(
                    failure.contains("Item is not valid"),
                    "unexpected failure: {}",
                    failure
                );
            }
            None => {
                if test.current_tick() == 15 {
                    assert!(!self.killed_item);
                    self.killed_item = true;
                    ctx.simulation.ecs.kill_entity(haul.item);
                }
            }
        }

        HookResult::TestSuccess
    }
}

impl HaulVariant for RemoveSourceContainerDuring {
    fn init(ctx: &HookContext) -> BoxedResult<Self> {
        ContainerToContainer::init(ctx).map(|nested| Self {
            nested,
            killed_container: false,
        })
    }

    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)> {
        self.nested.src_tgt(world)
    }

    fn validate_tick(
        &mut self,
        haul: &Haul<Self>,
        ctx: &HookContext,
        test: TestHelper,
    ) -> HookResult {
        match haul.has_activity_succeeded(ctx) {
            Some(EventResult::Success) => panic!("haul activity shouldn't have succeeded"),
            Some(EventResult::Failed(failure)) => {
                assert!(self.killed_container, "failed before container was killed");
                common::info!("test error: {}", failure);
                assert!(failure.contains("Invalid source container"));
                HookResult::TestSuccess
            }
            None => {
                if test.current_tick() == 5 {
                    assert!(!self.killed_container);

                    // queued for next tick
                    destroy_chest(ctx, self.nested.chest_a);
                    self.killed_container = true;
                }

                HookResult::KeepGoing
            }
        }
    }
}
impl HaulVariant for RemoveTargetContainerDuring {
    fn init(ctx: &HookContext) -> BoxedResult<Self> {
        ContainerToContainer::init(ctx).map(|nested| Self {
            nested,
            killed_container: false,
        })
    }

    fn src_tgt(&mut self, world: &mut EcsWorld) -> BoxedResult<(HaulTarget, HaulTarget)> {
        self.nested.src_tgt(world)
    }

    fn validate_tick(
        &mut self,
        haul: &Haul<Self>,
        ctx: &HookContext,
        test: TestHelper,
    ) -> HookResult {
        match haul.has_activity_succeeded(ctx) {
            Some(EventResult::Success) => panic!("haul activity shouldn't have succeeded"),
            Some(EventResult::Failed(failure)) => {
                assert!(self.killed_container, "failed before container was killed");
                common::info!("test error: {}", failure);
                assert!(failure.contains("Invalid target container"));
                HookResult::TestSuccess
            }
            None => {
                if test.current_tick() == 35 {
                    assert!(!self.killed_container);

                    // queued for next tick
                    destroy_chest(ctx, self.nested.chest_b);
                    self.killed_container = true;
                }

                HookResult::KeepGoing
            }
        }
    }
}

declare_test!(
    Haul<ContainerToContainer>
    Haul<ContainerToPosition>
    Haul<PositionToContainer>
    Haul<PositionToPosition>
    Haul<RemoveItemDuring>
    Haul<RemoveTargetContainerDuring>
    Haul<RemoveSourceContainerDuring>
);
