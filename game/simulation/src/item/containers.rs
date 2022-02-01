use common::*;
use std::num::NonZeroU16;
use std::rc::Rc;
use unit::space::volume::Volume;
use unit::world::WorldPosition;

use crate::definitions::{BuilderError, DefinitionErrorKind};
use crate::ecs::*;
use crate::event::DeathReason;
use crate::{
    EntityEvent, EntityEventPayload, PhysicalComponent, Societies, SocietyHandle,
    TransformComponent,
};

use crate::item::stack::{EntityCopyability, ItemStackComponent, StackAdd, StackMigrationType};
use crate::item::{ContainerComponent, ItemStack, ItemStackError};
use crate::simulation::AssociatedBlockData;
use crate::string::StringCache;

#[derive(Debug, Error, Clone)]
pub enum ContainersError {
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

    #[error("{0} is not an item stack")]
    NotAStack(Entity),

    #[error("Can't split zero items from a stack")]
    ZeroStackSplit,

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

/// An item can be stored inside a homogeneous stack
#[derive(Component, EcsComponent, Clone, Debug)]
#[name("stackable")]
#[storage(HashMapStorage)]
pub struct StackableComponent {
    pub max_count: u16,
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
    ) -> Result<(), ContainersError> {
        let entity = self.build_entity(definition_name)?.spawn()?;

        if !self.has_component::<ContainerComponent>(entity) {
            return Err(ContainersError::BadDefinition);
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

    pub fn destroy_container(
        &mut self,
        pos: WorldPosition,
        reason: DeathReason,
    ) -> Result<(), ContainersError> {
        let world = self.voxel_world();
        let container_entity = match world.borrow_mut().remove_associated_block_data(pos) {
            Some(AssociatedBlockData::Container(e)) => e,
            other => {
                error!("destroyed container does not have proper associated entity";
                                "position" => %pos, "data" => ?other);
                return Err(ContainersError::BadEntity);
            }
        };

        debug!("destroying container"; "container" => container_entity, "pos" => %pos);
        self.0.kill_entity(container_entity, reason);

        Ok(())
    }

    pub fn set_container_communal(
        &mut self,
        container_entity: Entity,
        new_society: Option<SocietyHandle>,
    ) -> Result<(), ContainersError> {
        let mut container = self
            .component_mut::<ContainerComponent>(container_entity)
            .map_err(|_| ContainersError::BadEntity)?;

        let prev_communal = match (container.communal(), new_society) {
            (prev, Some(society)) => {
                // resolve new society
                let society = self
                    .resource_mut::<Societies>()
                    .society_by_handle_mut(society)
                    .ok_or(ContainersError::BadSociety(society))?;

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
                    .ok_or(ContainersError::BadSociety(prev))?;

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
    /// stack, should be stackable, and have a transform and physical. Returns new stack entity
    pub fn convert_to_stack(&self, item: Entity) -> Result<Entity, ContainersError> {
        let stack_size;

        {
            // ensure stackable
            let stackables = self.read_storage::<StackableComponent>();
            let physicals = self.read_storage::<PhysicalComponent>();
            let transforms = self.read_storage::<TransformComponent>();

            let (stackable, _, _) = self
                .0
                .components(item, (&stackables, &physicals, &transforms))
                .ok_or(ItemStackError::NotStackable(item))?;

            stack_size =
                NonZeroU16::new(stackable.max_count).ok_or(ItemStackError::ZeroStackability)?;

            // ensure not already in a stack or container
            if self.has_component::<ContainedInComponent>(item) {
                return Err(ItemStackError::AlreadyStacked.into());
            }
        }

        // create stack with item already inside
        let stack_container = {
            let stack = ItemStack::new_with_item(
                stack_size,
                item,
                EntityCopyability::for_entity(self.0, item),
                self.0,
            )?;
            let entity = Entity::from(
                self.0
                    .create_entity()
                    .with(ItemStackComponent { stack })
                    .build(),
            );

            self.0.copy_components_to(item, entity).unwrap(); // both entities are definitely alive
            self.on_stack_creation(entity);
            entity
        };

        // sort out components
        let physicals = self.0.read_storage::<PhysicalComponent>();
        let item_volume = item.get(&physicals).unwrap().volume; // just copied

        self.on_successful_stack_addition(
            StackAdd::Distinct,
            stack_container,
            None,
            item,
            item_volume,
        );
        debug!(
            "created stack {stack} from item {item}",
            stack = stack_container,
            item = item
        );
        Ok(stack_container)
    }

    /// Item is checked for homogeneity with the stack
    pub fn add_to_stack(&self, stack: Entity, item: Entity) -> Result<(), ContainersError> {
        let copyability = EntityCopyability::for_entity(self.0, item);

        let stackables = self.read_storage::<StackableComponent>();
        let transforms = self.read_storage::<TransformComponent>();
        let contained = self.read_storage::<ContainedInComponent>();
        let mut stacks = self.0.write_storage::<ItemStackComponent>();
        let mut physicals = self.write_storage::<PhysicalComponent>();

        // ensure stackable, in the world, not already stacked
        let (_, physical, _, _) = self
            .0
            .components(
                item,
                (&stackables, &mut physicals, &transforms, !&contained),
            )
            .ok_or(ItemStackError::NotStackable(item))?;

        drop(transforms);
        drop(contained);

        let volume = physical.volume;

        // get stack
        let (stack_physical, stack_comp) = self
            .0
            .components(stack, (&mut physicals, &mut stacks))
            .ok_or(ContainersError::NotAStack(stack))?;

        // try to add
        let added = stack_comp.stack.try_add(item, copyability, self.0)?;

        drop(stacks);

        debug!("added {item} to stack {stack}", item = item, stack = stack);
        self.on_successful_stack_addition(added, stack, Some(stack_physical), item, volume);
        Ok(())
    }

    /// Returns the split stack, which may be the same as the given stack
    pub fn split_stack(&self, stack_entity: Entity, n: u16) -> Result<Entity, ContainersError> {
        let n = NonZeroU16::new(n).ok_or(ContainersError::ZeroStackSplit)?;

        // calculate ops for moving items to new stack
        let mut ops = SmallVec::<[_; 8]>::new();
        let new_stack = {
            let mut src_stack = self
                .0
                .component_mut::<ItemStackComponent>(stack_entity)
                .map_err(|_| ContainersError::NotAStack(stack_entity))?;

            match src_stack.stack.split_off(n, &mut ops)? {
                Some(stack) => stack,
                None => {
                    // reuse same stack
                    return Ok(stack_entity);
                }
            }
        };

        // spawn new stack entity
        let new_stack = Entity::from(
            self.0
                .create_entity()
                .with(ItemStackComponent { stack: new_stack })
                .build(),
        );
        self.0.copy_components_to(stack_entity, new_stack).unwrap(); // both entities are definitely alive
        self.on_stack_creation(stack_entity);

        // move items over
        let mut moved_total_volume = Volume::new(0);
        trace!("stack split operations"; "from" => stack_entity, "to" => new_stack, "ops" => ?ops);
        let mut entity_replacements = SmallVec::<[_; 4]>::new();
        for op in ops {
            let n = match op.ty {
                StackMigrationType::MoveDistinct => {
                    let _ = self
                        .0
                        .add_now(op.item, ContainedInComponent::StackOf(new_stack));
                    1
                }
                StackMigrationType::Move(n) => {
                    let _ = self
                        .0
                        .add_now(op.item, ContainedInComponent::StackOf(new_stack));
                    n.get()
                }
                StackMigrationType::Copy(n) => {
                    let orig_item = op.item;
                    let new_item = Entity::from(self.0.create_entity().build());
                    self.0.copy_components_to(orig_item, new_item).unwrap(); // both entities are definitely alive

                    // update its contained stack
                    let _ = self
                        .0
                        .add_now(new_item, ContainedInComponent::StackOf(new_stack));

                    // replace entity in the src stack
                    entity_replacements.push((orig_item, new_item));

                    n.get()
                }
            };

            let item_phys = self
                .0
                .component::<PhysicalComponent>(op.item)
                .expect("item should have physical");

            let total_volume = Volume::new(item_phys.volume.get() * n);
            moved_total_volume += total_volume;

            self.0.post_event(EntityEvent {
                subject: op.item,
                payload: EntityEventPayload::JoinedStack(new_stack),
            });
        }

        let mut stack = self
            .0
            .component_mut::<ItemStackComponent>(new_stack)
            .unwrap(); // is definitely a stack

        // tweak volumes
        let mut physicals = self.0.write_storage::<PhysicalComponent>();

        let src_phys = stack_entity.get_mut(&mut physicals).unwrap(); // definitely present
        src_phys.volume -= moved_total_volume;
        trace!("reduced source stack volume"; "stack" => stack_entity, "volume" => ?src_phys.volume);
        debug_assert!(src_phys.volume.get() > 0);

        let dst_phys = new_stack.get_mut(&mut physicals).unwrap(); // definitely present
        dst_phys.volume = moved_total_volume;
        trace!("set destination stack volume"; "stack" => new_stack, "volume" => ?dst_phys.volume);

        for (orig, replacement) in entity_replacements {
            if !stack.stack.replace_entity(orig, replacement) {
                warn!("failed to replace copied item entity in stack"; "stack" => new_stack,
                "notfound" => orig, "replacement" => replacement);
            }
        }

        Ok(new_stack)
    }

    fn on_stack_creation(&self, stack: Entity) {
        // adjust stack name, terribly hacky and temporary string manipulation
        if let Ok(mut name) = self.0.component_mut::<KindComponent>(stack) {
            name.make_stack();
        }
    }

    /// stack_physical should be None for the initial creation
    fn on_successful_stack_addition(
        &self,
        add: StackAdd,
        stack: Entity,
        stack_physical: Option<&mut PhysicalComponent>,
        item: Entity,
        item_volume: Volume,
    ) {
        self.0.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::JoinedStack(stack),
        });

        match add {
            StackAdd::Distinct => {
                // item is no longer free in the world
                let _ = self.0.remove_now::<TransformComponent>(item);

                // item is part of a stack
                let prev = self.0.add_now(item, ContainedInComponent::StackOf(stack));
                debug_assert!(matches!(prev, Ok(None)), "already had ContainedInComponent");

                // TODO unselect item
            }
            StackAdd::CollapsedIntoOther => {
                // item is identical to another, destroy
                self.0
                    .kill_entity(item, DeathReason::CollapsedIntoIdenticalInStack);
            }
        }

        if let Some(phys) = stack_physical {
            // increase item physical volume
            phys.volume += item_volume;
        }
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

    /// If the item is freestanding in the world and can be interacted with
    pub fn is_in_world(&self) -> bool {
        match self {
            ContainedInComponent::StackOf(_) => true,
            ContainedInComponent::Container(_) | ContainedInComponent::InventoryOf(_) => false,
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
    fn construct(
        values: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let max_count = values.get_int("max_count")?;
        Ok(Rc::new(Self { max_count }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

register_component_template!("stackable", StackableComponent);
