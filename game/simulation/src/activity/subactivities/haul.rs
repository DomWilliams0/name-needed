use common::*;

use crate::activity::activity::{ActivityFinish, ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::{Entity, WorldExt};
use crate::{ComponentWorld, InventoryComponent, PhysicalComponent, TransformComponent};

use crate::event::{EntityEvent, EntityEventPayload};
use crate::item::{ContainerError, EndHaulBehaviour, HaulType, HaulableItemComponent};

/// Handles holding of an item in the hauler's hands. No moving
#[derive(Debug)]
pub struct HaulSubActivity {
    thing: Entity,
}

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

    #[error("Invalid container entity for haul target")]
    BadContainer,

    #[error("Hauler doesn't have a transform")]
    BadHauler,

    #[error("Container operation failed: {0}")]
    Container(#[from] ContainerError),
}

impl HaulSubActivity {
    pub fn new(entity: Entity) -> Self {
        HaulSubActivity { thing: entity }
    }
}

impl SubActivity for HaulSubActivity {
    fn init(&self, ctx: &mut ActivityContext) -> ActivityResult {
        let hauler = ctx.entity;
        let item = self.thing;

        ctx.updates.queue("haul item", move |world| {
            let mut do_haul = || -> Result<Entity, HaulError> {
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
                    if item.get(&transforms).is_some() {
                        // not needed, item already has a transform
                        None
                    } else {
                        let transform = hauler.get(&transforms).ok_or(HaulError::BadHauler)?;
                        Some(transform.position)
                    }
                };

                // ensure hauler is close enough to haulee
                if cfg!(debug_assertions) {
                    let transforms = world.read_storage::<TransformComponent>();
                    let hauler_pos = hauler.get(&transforms).unwrap().position;
                    let haulee_pos = item.get(&transforms).unwrap().position;

                    assert!(
                        hauler_pos.is_almost(&haulee_pos, 3.0),
                        "{} is trying to haul {} but they are too far apart (hauler at {}, item at {}, distance is {:?}",
                        hauler,
                        item,
                        hauler_pos,
                        haulee_pos,
                        hauler_pos.distance2(haulee_pos).sqrt()
                    );
                }

                // everything has been checked, no more errors past this point

                // fill equip slots
                slots.fill(item, volume, size);

                // add components
                world
                    .helpers_comps()
                    .begin_haul(item, hauler, hauler_pos, HaulType::CarryAlone);

                // TODO apply slowness effect to holder
                Ok(hauler)
            };

            let result = do_haul();
            world.post_event(EntityEvent {
                subject: item,
                payload: EntityEventPayload::Hauled(result),
            });

            Ok(())
        });

        // TODO subscribe to container being destroyed

        // nothing else to do here, goto sub activity will handle the actual movement
        ActivityResult::Blocked
    }

    /// Only fails if holder has no inventory component
    fn on_finish(&self, finish: &ActivityFinish, ctx: &mut ActivityContext) -> BoxedResult<()> {
        let hauler = ctx.entity;
        let item = self.thing;
        let interrupted = matches!(finish, ActivityFinish::Interrupted);

        ctx.updates.queue("stop hauling item", move |world| {
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

            // TODO remove slowness effect if any

            Ok(())
        });

        Ok(())
    }

    fn exertion(&self) -> f32 {
        // TODO depends on the weight of the item(s)
        1.5
    }
}

impl Display for HaulSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Hauling {}", self.thing)
    }
}
