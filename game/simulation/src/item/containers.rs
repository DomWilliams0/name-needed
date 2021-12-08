use common::*;
use std::num::NonZeroU16;
use unit::space::volume::Volume;
use unit::world::WorldPosition;

use crate::definitions::{BuilderError, DefinitionErrorKind};
use crate::ecs::*;
use crate::{NameComponent, PhysicalComponent, Societies, SocietyHandle, TransformComponent};

use crate::item::stack::ItemStackComponent;
use crate::item::{ContainerComponent, ItemStack, ItemStackError};
use crate::simulation::AssociatedBlockData;

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

    #[error("Item is not stackable: {0}")]
    NonStackableItem(ComponentGetError),

    #[error("Item stackability must be > 0")]
    InvalidStackSize,

    #[error("Item is already stacked or in a container")]
    AlreadyStacked,

    #[error("Entity is not a stack: {0}")]
    NotAStack(ComponentGetError),

    #[error("Stack error: {0}")]
    StackError(#[from] ItemStackError),
}

/// An item is inside a container
#[derive(Component, EcsComponent, Clone, Debug)]
#[name("contained")]
#[storage(DenseVecStorage)]
#[clone(disallow)]
pub enum ContainedInComponent {
    Container(Entity),
    InventoryOf(Entity),
    StackOf(Entity),
}

/// An item can be stored inside a homogenous stack
#[derive(Component, EcsComponent, Clone, Debug)]
#[name("stackable")]
#[storage(HashMapStorage)]
pub struct StackableComponent {
    max_count: u16,
}

#[derive(common::derive_more::Deref, common::derive_more::DerefMut)]
pub struct EcsExtContainers<'w>(&'w EcsWorld);

impl EcsWorld {
    /// Helper methods to work with container entities
    pub fn helpers_containers(&self) -> EcsExtContainers {
        EcsExtContainers(self)
    }
}

impl EcsExtContainers<'_> {
    pub fn create_container_voxel(
        &mut self,
        pos: WorldPosition,
        definition_name: &'static str,
    ) -> Result<(), ContainerError> {
        let entity = self.build_entity(definition_name)?.spawn()?;

        if !self.has_component::<ContainerComponent>(entity) {
            return Err(ContainerError::BadDefinition);
        }

        let _ = self.add_now(entity, TransformComponent::new(pos.centred()));

        debug!("spawned new container entity"; "entity" => entity,
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

        debug!("destroying container"; "container" => container_entity, "pos" => %pos);
        self.0.kill_entity(container_entity);

        Ok(())
    }

    pub fn set_container_communal(
        &mut self,
        container_entity: Entity,
        new_society: Option<SocietyHandle>,
    ) -> Result<(), ContainerError> {
        let mut container = self
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

        info!("set container to communal"; "container" => container_entity,
              "society" => ?new_society, "previous" => ?prev_communal);
        Ok(())
    }

    /// Creates a new stack at the given entity's location and puts it inside. Item should not be a
    /// stack, should be stackable, and have a transform. Returns new stack entity
    pub fn convert_to_stack(&self, item: Entity) -> Result<Entity, ContainerError> {
        // ensure stackable
        let stack_size = self
            .0
            .component::<StackableComponent>(item)
            .map_err(ContainerError::NonStackableItem)
            .and_then(|comp| {
                NonZeroU16::new(comp.max_count).ok_or(ContainerError::InvalidStackSize)
            })?;

        // ensure not already in a stack or container
        if self.0.component::<ContainedInComponent>(item).is_ok() {
            return Err(ContainerError::AlreadyStacked);
        }

        // create stack
        let stack_container = {
            let entity = Entity::from(
                self.0
                    .create_entity()
                    .with(ItemStackComponent {
                        stack: ItemStack::new(stack_size),
                    })
                    .build(),
            );

            self.0.copy_components_to(item, entity);

            // adjust stack name, terribly hacky and temporary string manipulation
            if let Ok(mut name) = self.0.component_mut::<NameComponent>(entity) {
                // TODO use an enum variant StackOf in NameComponent
                name.0 = format!("Stack of {}", name.0);
            }

            entity
        };

        // kill on failure
        let bomb = EntityBomb::new(stack_container, self.0);

        let mut physicals = self.0.write_storage::<PhysicalComponent>();
        let mut stacks = self.0.write_storage::<ItemStackComponent>();

        let item_volume = item.get(&physicals).unwrap().volume; // just copied
        let (stack_physical, stack) = self
            .0
            .components(stack_container, (&mut physicals, &mut stacks))
            .unwrap(); // just added

        // attempt to put item inside
        match stack.stack.try_add(item) {
            Ok(_) => {
                // success
                self.on_successful_stack_addition(
                    stack_container,
                    stack_physical,
                    item,
                    item_volume,
                );
            }
            Err(err) => {
                warn!("failed to create stack for item"; "err" => %err);

                // redundant stack entity will be destroyed on return
                return Err(ContainerError::StackError(err));
            }
        }

        debug!("created stack from item"; "item" => item, "stack" => stack_container);
        bomb.defuse();
        Ok(stack_container)
    }

    pub fn add_to_stack(&self, stack: Entity, item: Entity) -> Result<(), ContainerError> {
        // ensure stackable, in the world, not already stacked
        if let Err(err) = self.0.component::<StackableComponent>(item) {
            return Err(ContainerError::NonStackableItem(err));
        }

        if let Err(err) = self.0.component::<TransformComponent>(item) {
            return Err(ContainerError::NonStackableItem(err));
        }

        if self.0.component::<ContainedInComponent>(item).is_ok() {
            return Err(ContainerError::AlreadyStacked);
        }

        let mut physicals = self.0.write_storage::<PhysicalComponent>();
        let mut stacks = self.0.write_storage::<ItemStackComponent>();

        let item_volume = item
            .get(&physicals)
            .ok_or(ContainerError::NonStackableItem(
                ComponentGetError::NoSuchComponent(item, "physical"),
            ))?
            .volume;

        let (stack_physical, stack_comp) = self
            .0
            .components(stack, (&mut physicals, &mut stacks))
            .ok_or(ContainerError::NotAStack(
                ComponentGetError::NoSuchComponent(stack, "physical or item stack"),
            ))?;

        // try to add
        stack_comp.stack.try_add(item)?;

        // success - remove components
        self.on_successful_stack_addition(stack, stack_physical, item, item_volume);
        Ok(())
    }

    fn on_successful_stack_addition(
        &self,
        stack: Entity,
        stack_physical: &mut PhysicalComponent,
        item: Entity,
        item_volume: Volume,
    ) {
        // item is no longer in the world
        let _ = self.0.remove_now::<TransformComponent>(item);

        // item is part of a stack
        let _ = self.0.add_now(item, ContainedInComponent::StackOf(stack));
        // TODO post event? does entering a stack count as destructive? the transform is gone at least
        // TODO unselect item

        // increase item physical volume
        stack_physical.volume += item_volume;
    }
}

impl ContainedInComponent {
    /// Container or inventory holder or stack
    pub fn entity(&self) -> Entity {
        match self {
            ContainedInComponent::Container(e)
            | ContainedInComponent::InventoryOf(e)
            | ContainedInComponent::StackOf(e) => *e,
        }
    }
}

impl Display for ContainedInComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainedInComponent::Container(c) => write!(f, "container {}", c),
            ContainedInComponent::InventoryOf(e) => write!(f, "inventory of {}", e),
            ContainedInComponent::StackOf(s) => write!(f, "stack {}", s),
        }
    }
}

impl<V: Value> ComponentTemplate<V> for StackableComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let max_count = values.get_int("max_count")?;
        Ok(Box::new(Self { max_count }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

register_component_template!("stackable", StackableComponent);
