use common::*;
use unit::world::WorldPosition;

use crate::definitions::{BuilderError, DefinitionErrorKind};
use crate::ecs::*;
use crate::item::inventory::HeldEntity;
use crate::item::ContainerComponent;
use crate::simulation::AssociatedBlockData;
use crate::{Societies, SocietyHandle, TransformComponent};

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error("Definition error: {0}")]
    Definition(#[from] DefinitionErrorKind),

    #[error("Definition does not include entity container component")]
    BadDefinition,

    #[error("Builder error: {0}")]
    Builder(#[from] BuilderError),

    #[error("Container block lacks associated container entity")]
    BadEntity,

    #[error("No such society with handle {0:?}")]
    BadSociety(SocietyHandle),
}

/// An item is inside a container
#[derive(Component, EcsComponent, Clone, Debug)]
#[name("contained")]
#[storage(DenseVecStorage)]
pub enum ContainedInComponent {
    Container(Entity),
    InventoryOf(Entity),
}

#[derive(common::derive_more::Deref, common::derive_more::DerefMut)]
pub struct EcsExtContainers<'w>(&'w mut EcsWorld);

impl EcsWorld {
    /// Helper methods to work with container entities
    pub fn helpers_containers(&mut self) -> EcsExtContainers {
        EcsExtContainers(self)
    }
}

impl EcsExtContainers<'_> {
    pub fn create_container(
        &mut self,
        pos: WorldPosition,
        definition_name: &'static str,
    ) -> Result<(), ContainerError> {
        let entity = self.build_entity(definition_name)?.spawn()?;

        if !self.has_component::<ContainerComponent>(entity) {
            return Err(ContainerError::BadDefinition);
        }

        self.add_now(entity, TransformComponent::new(pos.centred()))
            .unwrap();

        debug!("spawned new container entity"; "entity" => E(entity),
            "definition" => definition_name, "pos" => %pos);

        let world = self.voxel_world();
        let old_data = world
            .borrow_mut()
            .set_associated_block_data(pos, AssociatedBlockData::Container(entity));
        debug_assert!(old_data.is_none());
        Ok(())
    }

    pub fn destroy_container(&mut self, pos: WorldPosition) -> Result<(), ContainerError> {
        let world = self.voxel_world();
        let container_entity = match world.borrow_mut().remove_associated_block_data(pos) {
            Some(AssociatedBlockData::Container(e)) => e,
            other => {
                error!("destroyed container does not have proper associated entity";
                                "position" => %pos, "data" => ?other);
                return Err(ContainerError::BadEntity);
            }
        };

        debug!("destroying container"; "container" => E(container_entity), "pos" => %pos);

        let mut container = self
            .remove_now::<ContainerComponent>(container_entity)
            .ok_or(ContainerError::BadEntity)?;

        // remove all items from container
        let mut rng = thread_rng();
        for HeldEntity { entity: item, .. } in container.container.remove_all() {
            self.helpers_comps().remove_from_container(item);

            // scatter items around
            let scatter_pos = {
                let offset_x = rng.gen_range(-0.3, 0.3);
                let offset_y = rng.gen_range(-0.3, 0.3);
                pos.centred() + (offset_x, offset_y, 0.0)
            };

            let _ = self.add_now(item, TransformComponent::new(scatter_pos));
        }

        // destroy container entity
        self.kill_entity(container_entity);

        Ok(())
    }

    pub fn set_container_communal(
        &mut self,
        container_entity: Entity,
        new_society: Option<SocietyHandle>,
    ) -> Result<(), ContainerError> {
        let container = self
            .component_mut::<ContainerComponent>(container_entity)
            .map_err(|_| ContainerError::BadEntity)?;

        let prev_communal = match (container.communal(), new_society) {
            (prev, Some(society)) => {
                // resolve new society
                let society = self
                    .resource_mut::<Societies>()
                    .society_by_handle_mut(society)
                    .ok_or(ContainerError::BadSociety(society))?;

                // update container first
                let prev_communal = container.make_communal(new_society);
                debug_assert_eq!(prev, prev_communal);

                // then add to society
                let result = society.add_communal_container(container_entity, self.0);
                debug_assert!(result);

                prev_communal
            }
            (Some(prev), None) => {
                // resolve old society
                let society = self
                    .resource_mut::<Societies>()
                    .society_by_handle_mut(prev)
                    .ok_or(ContainerError::BadSociety(prev))?;

                // remove from society
                let result = society.remove_communal_container(container_entity, self.0);
                debug_assert!(result);

                // then update container
                let prev_communal = container.make_communal(None);
                debug_assert_eq!(Some(prev), prev_communal);

                prev_communal
            }

            (None, None) => {
                // nop
                return Ok(());
            }
        };

        info!("set container to communal"; "container" => E(container_entity),
              "society" => ?new_society, "previous" => ?prev_communal);
        Ok(())
    }
}

impl ContainedInComponent {
    /// Container or inventory holder
    pub fn entity(&self) -> Entity {
        match self {
            ContainedInComponent::Container(e) | ContainedInComponent::InventoryOf(e) => *e,
        }
    }
}

impl Display for ContainedInComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainedInComponent::Container(c) => write!(f, "container {}", E(*c)),
            ContainedInComponent::InventoryOf(e) => write!(f, "inventory of {}", E(*e)),
        }
    }
}
