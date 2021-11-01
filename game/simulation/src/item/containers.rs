use common::*;
use unit::space::volume::Volume;
use unit::world::WorldPosition;

use crate::definitions::{BuilderError, DefinitionErrorKind};
use crate::ecs::*;
use crate::{
    NameComponent, PhysicalComponent, RenderComponent, Societies, SocietyHandle, TransformComponent,
};

use crate::item::inventory::HeldEntity;
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

        self.add_now(entity, TransformComponent::new(pos.centred()))
            .unwrap();

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
            .map_err(ContainerError::NonStackableItem)?
            .max_count;

        // ensure not already in a stack or container
        if self.0.component::<ContainedInComponent>(item).is_ok() {
            return Err(ContainerError::AlreadyStacked);
        }

        // create stack
        let stack_container = {
            let mut builder = self.0.create_entity().with(ItemStackComponent {
                stack: ItemStack::new(stack_size),
            });

            macro_rules! copy_component {
                ($comp:ty) => {
                    let comp_copy = self
                        .0
                        .component::<$comp>(item)
                        .map(|comp| <$comp>::clone(&comp))
                        .map_err(ContainerError::NonStackableItem)?;
                    builder = builder.with(comp_copy);
                };
            }

            // copy relevant components
            copy_component!(TransformComponent);
            copy_component!(RenderComponent);
            copy_component!(PhysicalComponent);

            // adjust name
            let new_name = self
                .0
                .component::<NameComponent>(item)
                .ok()
                .map(|name| NameComponent(format!("Stack of {}", name.0)));

            if let Some(name) = new_name {
                builder = builder.with(name);
            }

            Entity::from(builder.build())
        };

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

                // destroy redundant stack entity first
                self.0.kill_entity(stack_container);
                return Err(ContainerError::StackError(err));
            }
        }

        debug!("created stack from item"; "item" => item, "stack" => stack_container);
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
}

register_component_template!("stackable", StackableComponent);
