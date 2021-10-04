use common::*;
use unit::world::WorldPoint;

use crate::activity::activity2::{ActivityContext2, DistanceCheckResult};
use crate::activity::status::Status;
use crate::ecs::*;
use crate::event::prelude::*;
use crate::item::{ContainerError, EndHaulBehaviour, HaulType, HaulableItemComponent};
use crate::queued_update::QueuedUpdates;
use crate::{
    ComponentWorld, ContainedInComponent, ContainerComponent, EntityEvent, EntityEventPayload,
    InventoryComponent, PhysicalComponent, TransformComponent, WorldPosition,
};

#[derive(Debug, Error, Clone)]
pub enum HaulError {
    #[error("Item destroyed/moved by a destructive event")]
    Interrupted,

    #[error("Hauler has no inventory")]
    NoInventory,

    #[error("Hauler doesn't have enough free hands")]
    NotEnoughFreeHands,

    #[error("Item is not valid, haulable or physical")]
    BadItem,

    #[error("Invalid source container entity for haul")]
    BadSourceContainer,

    #[error("Invalid target container entity for haul")]
    BadTargetContainer,

    #[error("Hauler doesn't have a transform")]
    BadHauler,

    #[error("Container operation failed: {0}")]
    Container(#[from] ContainerError),

    #[error("Item is too far away to haul")]
    TooFar,

    #[error("Haul was cancelled")]
    Cancelled,
}

/// Handles the start (picking up) and finish (putting down), and fixing up components on abort
#[must_use]
pub struct HaulSubactivity2<'a> {
    ctx: &'a ActivityContext2,
    thing: Entity,
    complete: bool,
    needs_transform: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum HaulTarget {
    /// Put in/take from an accessible position
    // TODO worldpoint
    Position(WorldPosition),

    /// Put in/take out of a container
    Container(Entity),
}

// TODO depends on item size
const MAX_DISTANCE: f32 = 4.0;

struct StartHaulingStatus(HaulTarget);

struct StopHaulingStatus(Option<HaulTarget>);

impl<'a> HaulSubactivity2<'a> {
    /// Checks if close enough to start hauling
    pub async fn start_hauling(
        ctx: &'a ActivityContext2,
        thing: Entity,
        source: HaulTarget,
    ) -> Result<HaulSubactivity2<'a>, HaulError> {
        // check distance
        if let HaulTarget::Position(_) = source {
            match ctx.check_entity_distance(thing, MAX_DISTANCE.powi(2)) {
                DistanceCheckResult::NotAvailable => return Err(HaulError::BadItem),
                DistanceCheckResult::TooFar => return Err(HaulError::TooFar),
                DistanceCheckResult::InRange => {}
            }
        }

        ctx.update_status(StartHaulingStatus(source));

        // get item out of container if necessary

        // create instance now, so on drop/failure we can fix up the transform
        let mut subactivity = HaulSubactivity2 {
            ctx,
            thing,
            complete: false,
            needs_transform: false,
        };

        if let HaulTarget::Container(container) = source {
            subactivity.needs_transform = true;

            queue_container_removal(ctx.world().resource(), thing, container);

            // wait for removal and ensure it succeeded
            ctx.subscribe_to_specific_until(
                thing,
                EntityEventType::ExitedContainer,
                |evt| match evt {
                    EntityEventPayload::ExitedContainer(result) => {
                        if let Ok(exited_container) = result.as_ref() {
                            debug_assert_eq!(*exited_container, container);
                        }

                        Ok(result)
                    }
                    _ => Err(evt),
                },
            )
            .await
            .unwrap_or(Err(HaulError::Cancelled))?;
        }

        // start hauling, which will give the item a transform on success
        queue_haul_pickup(ctx.world().resource(), ctx.entity(), thing);

        // wait for event and ensure the haul succeeded
        ctx.subscribe_to_specific_until(thing, EntityEventType::Hauled, |evt| match evt {
            EntityEventPayload::Hauled(hauler, result) => {
                if hauler != ctx.entity() {
                    // someone else picked it up
                    return Err(EntityEventPayload::Hauled(hauler, result));
                }
                Ok(result)
            }
            // calling activity can handle other destructive events
            _ => Err(evt),
        })
        .await
        .unwrap_or(Err(HaulError::Cancelled))?;

        // defuse bomb, we have a transform now
        subactivity.needs_transform = false;
        Ok(subactivity)
    }

    /// If target is a location in the world, assumes we have already walked to it.
    /// This is the non-interrupted entrypoint
    pub async fn end_haul(&mut self, target: HaulTarget) -> Result<(), HaulError> {
        self.complete = true;
        self.end_haul_impl_sync(Some(target))?;

        if matches!(target, HaulTarget::Container(_)) {
            self.end_haul_impl_async_container(target).await?;
        }

        Ok(())
    }

    /// First part of end hauling that only supports dropping the item, not container. Call other
    /// async method after this to put in container
    fn end_haul_impl_sync(&self, target: Option<HaulTarget>) -> Result<(), HaulError> {
        let updates = self.ctx.world().resource();

        self.ctx.update_status(StopHaulingStatus(target));

        // fix up components next tick infallibly
        queue_stop_hauling(updates, self.thing, self.ctx.entity(), !self.complete);

        let target = match target {
            Some(tgt) if self.complete => {
                // preserve original
                tgt
            }
            _ => {
                // drop at hauler's feet because we were interrupted
                let hauler_pos = self
                    .ctx
                    .world()
                    .component::<TransformComponent>(self.ctx.entity())
                    .map_err(|_| HaulError::BadHauler)?
                    .position;
                HaulTarget::Position(hauler_pos.floor())
            }
        };

        // place haulee back into the world
        let item = self.thing;
        match target {
            HaulTarget::Position(pos) => {
                // drop the item in place
                // TODO don't always drop item in centre
                let pos = pos.centred();
                let needs_transform = self.needs_transform;

                updates.queue("drop hauled item", move |world| {
                    if let Ok(transform) = world.component_mut::<TransformComponent>(item) {
                        transform.reset_position(pos);
                    } else if needs_transform {
                        // add transform if missing
                        let _ = world.add_now(item, TransformComponent::new(pos));
                    }

                    Ok(())
                });
            }
            HaulTarget::Container(_) => {
                // do this in async end_haul_impl_async_container
            }
        }

        Ok(())
    }

    #[inline]
    async fn end_haul_impl_async_container(&self, target: HaulTarget) -> Result<(), HaulError> {
        let container_entity = match target {
            HaulTarget::Container(e) => e,
            _ => unreachable!("not a container target"),
        };

        // put the item in the container
        queue_put_into_container(
            self.ctx.world().resource(),
            self.thing,
            self.ctx.entity(),
            container_entity,
        );

        // wait for success
        self.ctx
            .subscribe_to_specific_until(self.thing, EntityEventType::EnteredContainer, |evt| {
                match evt {
                    EntityEventPayload::EnteredContainer(result) => Ok(result),
                    _ => Err(evt),
                }
            })
            .await
            .unwrap_or(Err(HaulError::Interrupted))?;

        // dont bother checking which container it entered, we just queued the right one

        Ok(())
    }
}

impl<'a> Drop for HaulSubactivity2<'a> {
    fn drop(&mut self) {
        if !self.complete {
            if let Err(err) = self.end_haul_impl_sync(None) {
                warn!("failed to abort haul subactivity: {}", err);
            }
        }
    }
}

impl Display for HaulTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HaulTarget::Position(pos) => Display::fmt(pos, f),
            HaulTarget::Container(container) => write!(f, "container {}", container),
        }
    }
}

impl HaulTarget {
    /// Creates from entity's current position or the container it is currently contained in
    pub fn with_entity(entity: Entity, world: &impl ComponentWorld) -> Option<Self> {
        if let Ok(ContainedInComponent::Container(container)) = world.component(entity) {
            Some(Self::Container(*container))
        } else if let Ok(transform) = world.component::<TransformComponent>(entity) {
            Some(Self::Position(transform.accessible_position()))
        } else {
            None
        }
    }

    pub fn target_position(&self, world: &impl ComponentWorld) -> Option<WorldPoint> {
        match self {
            HaulTarget::Position(pos) => Some(pos.centred()),
            HaulTarget::Container(container) => Self::position_of(world, *container),
        }
    }

    pub fn source_position(
        &self,
        world: &impl ComponentWorld,
        item: Entity,
    ) -> Result<WorldPoint, HaulError> {
        match self {
            HaulTarget::Position(_) => {
                // get current position instead of possibly outdated source position
                Self::position_of(world, item).ok_or(HaulError::BadItem)
            }
            HaulTarget::Container(container) => {
                Self::position_of(world, *container).ok_or(HaulError::BadSourceContainer)
            }
        }
    }

    // TODO explicit access side for container, e.g. front of chest
    fn position_of(world: &impl ComponentWorld, entity: Entity) -> Option<WorldPoint> {
        world
            .component::<TransformComponent>(entity)
            .ok()
            .map(|transform| transform.position)
    }
}

fn queue_container_removal(updates: &QueuedUpdates, item: Entity, container_entity: Entity) {
    updates.queue("remove item from container", move |world| {
        let do_remove = || -> Result<Entity, HaulError> {
            let container = world
                .component_mut::<ContainerComponent>(container_entity)
                .map_err(|_| HaulError::BadSourceContainer)?;

            // remove from container
            container.container.remove(item)?;

            // remove contained component
            world.helpers_comps().remove_from_container(item);

            // the item needs a transform to be added back, leave this until
            // the initialisation of the haul subactivity to avoid jerkiness
            Ok(container_entity)
        };

        let result = do_remove();
        world.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::ExitedContainer(result),
        });

        Ok(())
    });
}

fn queue_haul_pickup(updates: &QueuedUpdates, hauler: Entity, item: Entity) {
    updates.queue("haul item", move |world| {
        let do_haul = || -> Result<(), HaulError> {
            // check item is alive first, to ensure .insert() succeeds below
            if !world.is_entity_alive(item) {
                return Err(HaulError::BadItem);
            }

            // get item properties
            let (extra_hands, volume, size) = {
                let haulables = world.read_storage::<HaulableItemComponent>();
                let physicals = world.read_storage::<PhysicalComponent>();
                match world.components(item, (&haulables, &physicals)) {
                    Some((haulable, physical)) => {
                        (haulable.extra_hands, physical.volume, physical.size)
                    }
                    None => {
                        warn!("item is not haulable"; "item" => item);
                        return Err(HaulError::BadItem);
                    }
                }
            };

            debug!(
                "{entity} wants to haul {item} which needs {extra_hands} extra hands",
                entity = hauler,
                item = item,
                extra_hands = extra_hands
            );

            // get hauler inventory
            let inventory = world
                .component_mut::<InventoryComponent>(hauler)
                .map_err(|_| HaulError::NoInventory)?;

            // ensure hauler has enough free hands
            let mut slots = inventory
                .get_hauling_slots(extra_hands)
                .ok_or(HaulError::NotEnoughFreeHands)?;

            // get hauler position if needed
            let hauler_pos = {
                let transforms = world.read_storage::<TransformComponent>();
                if item.has(&transforms) {
                    // not needed, item already has a transform
                    None
                } else {
                    let transform = hauler.get(&transforms).ok_or(HaulError::BadHauler)?;
                    Some(transform.position)
                }
            };

            // everything has been checked, no more errors past this point

            // fill equip slots
            slots.fill(item, volume, size);

            // add components
            world
                .helpers_comps()
                .begin_haul(item, hauler, hauler_pos, HaulType::CarryAlone);

            Ok(())
        };

        let result = do_haul();
        world.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::Hauled(hauler, result),
        });

        Ok(())
    });
}

fn queue_stop_hauling(updates: &QueuedUpdates, item: Entity, hauler: Entity, interrupted: bool) {
    updates.queue("stop hauling item", move |world| {
        // remove components from item
        let behaviour = world.helpers_comps().end_haul(item, interrupted);

        let count = match behaviour {
            EndHaulBehaviour::Drop => {
                // free holder's hands
                let inventory = world
                    .component_mut::<InventoryComponent>(hauler)
                    .map_err(|_| HaulError::NoInventory)?;

                inventory.remove_item(item)
            }
            EndHaulBehaviour::KeepEquipped => {
                // dont remove from inventory
                0
            }
        };

        debug!(
            "{hauler} stopped hauling {item}, removed from {slots} slots",
            hauler = hauler,
            item = item,
            slots = count;
            "behaviour" => ?behaviour,
        );

        Ok(())
    });
}

fn queue_put_into_container(
    updates: &QueuedUpdates,
    item: Entity,
    hauler: Entity,
    container_entity: Entity,
) {
    updates.queue("put hauled item into container", move |world| {
        let do_put = || -> Result<Entity, HaulError> {
            let item_physical = world
                .component::<PhysicalComponent>(item)
                .map_err(|_| HaulError::BadItem)?;

            let container = world
                .component_mut::<ContainerComponent>(container_entity)
                .map_err(|_| HaulError::BadTargetContainer)?;

            container
                .container
                .add_with(item, item_physical.volume, item_physical.size)?;

            // added to container successfully, do component dance
            world
                .helpers_comps()
                .add_to_container(item, ContainedInComponent::Container(container_entity));

            trace!("put item into container"; "item" => item,
                "container" => container_entity, "hauler" => hauler
            );

            Ok(container_entity)
        };

        let payload = do_put();
        world.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::EnteredContainer(payload),
        });

        Ok(())
    });
}

impl Status for StartHaulingStatus {
    fn exertion(&self) -> f32 {
        // TODO depends on weight of item
        2.0
    }
}

impl Display for StartHaulingStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            HaulTarget::Position(_) => f.write_str("Picking up item"),
            HaulTarget::Container(_) => f.write_str("Taking item out of container"),
        }
    }
}

impl Display for StopHaulingStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            None | Some(HaulTarget::Position(_)) => f.write_str("Dropping item"),
            Some(HaulTarget::Container(_)) => f.write_str("Putting item into container"),
        }
    }
}

impl Status for StopHaulingStatus {
    fn exertion(&self) -> f32 {
        // TODO depends on weight of item
        2.0
    }
}
