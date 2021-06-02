use unit::world::WorldPoint;

use crate::ecs::{EcsWorld, Entity};
use crate::{ComponentWorld, TransformComponent};

use crate::item::{ContainedInComponent, EndHaulBehaviour, HaulType, HauledItemComponent};

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

        self.add_now(haulee, HauledItemComponent::new(hauler, haul_type))
            .unwrap(); // haulee is alive

        self.add_now(haulee, ContainedInComponent::InventoryOf(hauler))
            .unwrap();

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
    /// Returns actual behaviour done
    pub fn end_haul(&mut self, haulee: Entity, interrupted: bool) -> EndHaulBehaviour {
        // remove haul component unconditionally
        let hauled = self.remove_now::<HauledItemComponent>(haulee);
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
                let contained_in = self
                    .component::<ContainedInComponent>(haulee)
                    .unwrap()
                    .clone();
                self.add_to_container(haulee, contained_in);
            }
        };

        behaviour
    }

    /// Removes ContainedInComponent
    pub fn remove_from_container(&mut self, item: Entity) {
        let result = self.remove_now::<ContainedInComponent>(item);
        debug_assert!(result.is_some(), "{} didnt have contained component", item);
    }

    /// Removes TransformComponent and adds given ContainedInComponent
    pub fn add_to_container(&mut self, item: Entity, container: ContainedInComponent) {
        debug_assert!(self.is_entity_alive(item));

        let _ = self.remove_now::<TransformComponent>(item);
        self.add_now(item, container).unwrap();
    }
}
