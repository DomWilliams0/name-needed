use unit::world::WorldPoint;

use crate::build::{ConsumedMaterialForJobComponent, ReservedMaterialComponent};
use crate::ecs::{EcsWorld, Entity, WorldExt};
use crate::{ComponentWorld, ContainersError, TransformComponent};
use common::*;

use crate::item::{ContainedInComponent, EndHaulBehaviour, HaulType, HauledItemComponent};
use crate::job::{BuildThingError, BuildThingJob, MaterialReservation, SocietyJobHandle};

#[derive(common::derive_more::Deref, common::derive_more::DerefMut)]
pub struct EcsExtComponents<'w>(&'w EcsWorld);

impl EcsWorld {
    /// Helper methods to add and remove components for given actions
    pub fn helpers_comps(&self) -> EcsExtComponents {
        EcsExtComponents(self)
    }
}

impl EcsExtComponents<'_> {
    /// Adds HauledItemComponent and ContainedInComponent to haulee unconditionally, and
    /// TransformComponent if hauler_pos is Some
    ///
    /// Panics if haulee is not alive
    pub fn begin_haul(
        &self,
        haulee: Entity,
        hauler: Entity,
        hauler_pos: Option<WorldPoint>,
        haul_type: HaulType,
    ) {
        debug_assert!(self.is_entity_alive(haulee));

        let _ = self.add_now(haulee, HauledItemComponent::new(hauler, haul_type));
        let _ = self.add_now(haulee, ContainedInComponent::InventoryOf(hauler));

        // add transform to the haulee if it doesn't already have one
        if let Some(hauler_pos) = hauler_pos {
            debug_assert!(
                !self.has_component::<TransformComponent>(haulee),
                "haulee already has transform"
            );
            let _ = self.add_now(haulee, TransformComponent::new(hauler_pos));
        }
    }

    /// Removes HauledItemComponent and possibly ContainedInComponent, depending on if it was
    /// interrupted and the specified interruption behaviour.
    ///
    /// Returns None if the old hauler does not match the given hauler, otherwise the actual behaviour done
    pub fn end_haul(
        &mut self,
        haulee: Entity,
        hauler: Entity,
        interrupted: bool,
    ) -> Option<EndHaulBehaviour> {
        let mut hauleds = self.0.write_storage::<HauledItemComponent>();
        if let Some(hauled) = hauleds.get(haulee.into()) {
            if hauled.hauler != hauler {
                // something changed, abort the abort
                return None;
            }
        }

        // remove haul component unconditionally
        let hauled = hauleds.remove(haulee.into());
        let behaviour = match (interrupted, hauled) {
            (true, Some(hauled)) => {
                // interrupted and it was actually being hauled, nice
                hauled.interrupt_behaviour
            }
            _ => EndHaulBehaviour::default(),
        };

        match behaviour {
            EndHaulBehaviour::Drop => {
                let _ = self.remove_now::<ContainedInComponent>(haulee);
            }
            EndHaulBehaviour::KeepEquipped => {
                if let Ok(contained_in) = self.component::<ContainedInComponent>(haulee).as_deref()
                {
                    self.add_to_container(haulee, contained_in.clone());
                }
            }
        };

        Some(behaviour)
    }

    /// Removes ContainedInComponent
    pub fn remove_from_container(&mut self, item: Entity) {
        let result = self.remove_now::<ContainedInComponent>(item);
        debug_assert!(result.is_some(), "{} didnt have contained component", item);
    }

    /// Removes TransformComponent and adds given ContainedInComponent.
    /// Item must still be added to ContainerComponent!!
    pub fn add_to_container(&mut self, item: Entity, container: ContainedInComponent) {
        debug_assert!(self.is_entity_alive(item));

        let _ = self.remove_now::<TransformComponent>(item);
        let _ = self.add_now(item, container);
    }

    pub fn reserve_material_for_job(
        &mut self,
        material: Entity,
        job: SocietyJobHandle,
    ) -> Result<(), ReservationError> {
        // find job in society and try to reserve
        let (surplus, remaining) = job
            .resolve_and_cast_mut(self.0.resource(), |build_job: &mut BuildThingJob| {
                build_job.add_reservation(material, self.0)
            })
            .ok_or(ReservationError::InvalidJob(job))??;

        match surplus {
            MaterialReservation::ConsumeAll(n) => {
                debug!("reserving material for build job"; "material" => material, "job" => ?job, "n" => n, "remaining" => remaining);
            }
            MaterialReservation::Surplus { surplus, reserved } => {
                // drop surplus
                debug!("reserving {n} material for build job with {surplus} surplus to be split into new stack",
                    n = reserved, surplus = surplus; "job" => ?job);
                let new_stack = self
                    .0
                    .helpers_containers()
                    .split_stack(material, surplus)
                    .map_err(ReservationError::DropSurplus)?;

                // drop it at a slight offset
                if new_stack != material {
                    let mut transform = self
                        .0
                        .component_mut::<TransformComponent>(new_stack)
                        .expect("transform expected");

                    let pos = {
                        let mut rand = thread_rng();
                        transform
                            .position
                            .modify_x(|x| x + rand.gen_range(-1.0, 1.0))
                            .modify_y(|y| y + rand.gen_range(-1.0, 1.0))
                    };

                    transform.reset_position(pos)
                }
            }
        }

        // no more failures, now we can add the reservation to the item
        let _ = self
            .0
            .add_now(material, ReservedMaterialComponent { build_job: job });

        Ok(())
    }

    pub fn consume_materials_for_job(&mut self, materials: &[Entity]) {
        let mut transforms = self.0.write_storage::<TransformComponent>();
        let mut consumeds = self.0.write_storage::<ConsumedMaterialForJobComponent>();
        for material in materials {
            let material = specs::Entity::from(*material);
            transforms.remove(material);
            let _ = consumeds.insert(material, ConsumedMaterialForJobComponent::default());
        }
    }

    pub fn unconsume_materials_for_job(&mut self, materials: &[Entity], job_pos: WorldPoint) {
        let mut transforms = self.0.write_storage::<TransformComponent>();
        let mut consumeds = self.0.write_storage::<ConsumedMaterialForJobComponent>();
        for material in materials {
            let material = specs::Entity::from(*material);
            // TODO scatter around
            let _ = transforms.insert(material, TransformComponent::new(job_pos));
            let _ = consumeds.remove(material);
        }
    }
}

#[derive(Debug, Error)]
pub enum ReservationError {
    #[error("Job used for society job material reservation not found: {0:?}")]
    JobNotFound(SocietyJobHandle),

    #[error("Job used for society material reservation is not a build job: {0:?}")]
    InvalidJob(SocietyJobHandle),

    #[error("Failed to reserve material: {0}")]
    BuildJob(#[from] BuildThingError),

    #[error("Failed to drop surplus materials: {0}")]
    DropSurplus(ContainersError),
}
