use crate::definitions::DefinitionNameComponent;
use crate::ecs::*;
use common::*;
use std::num::NonZeroU16;

#[derive(Debug, Error)]
pub enum ItemStackError {
    #[error("Item stack is full")]
    Full,

    #[error("{0} is not homogenous with the rest of the stack")]
    NotHomogenous(Entity),

    #[error("Cannot calculate homogeneity for {0}")]
    CantGetHomogeneity(Entity),
}

/// Defines the criteria for allowing an item into a stack
#[derive(Debug)]
struct StackHomogeneity {
    // TODO use a better way than hacky definition names
    definition: String,
}

/// A stack of homogenous entities that are still distinct but stacked together. Examples might
/// be a stack of arrows holds 10xsteel_arrows, 12xwood_arrows
#[derive(Debug)]
pub struct ItemStack {
    contents: Vec<StackedEntity>,
    total_count: u16,
    homogeneity: StackHomogeneity,
}

/// A stack of IDENTICAL entities
#[derive(Debug)] // TODO implement manually e.g. "1xentity"
struct StackedEntity {
    entity: Entity,
    count: u16,
}

/// A homogenous stack of items
#[derive(Component, EcsComponent, Debug)]
#[name("item-stack")]
#[storage(DenseVecStorage)]
#[clone(disallow)]
pub struct ItemStackComponent {
    pub stack: ItemStack,
}

impl ItemStack {
    pub fn new_with_item(
        max_size: NonZeroU16,
        first_item: Entity,
        world: &EcsWorld,
    ) -> Result<Self, ItemStackError> {
        let homogeneity = StackHomogeneity::from_entity(first_item, world)
            .ok_or(ItemStackError::CantGetHomogeneity(first_item))?;

        let mut stack = ItemStack {
            contents: Vec::with_capacity(max_size.get() as usize),
            total_count: 0,
            homogeneity,
        };

        stack.add_internal(first_item, world);
        Ok(stack)
    }

    /// Does not touch any components, ensure they are updated
    pub fn try_add(&mut self, entity: Entity, world: &EcsWorld) -> Result<(), ItemStackError> {
        if self.is_full() {
            Err(ItemStackError::Full)
        } else if !self.homogeneity.matches(entity, world) {
            Err(ItemStackError::NotHomogenous(entity))
        } else {
            Ok(self.add_internal(entity, world))
        }
    }

    /// Capacity and homogeneity must have been checked
    fn add_internal(&mut self, entity: Entity, world: &EcsWorld) {
        debug_assert!(!self.is_full());
        debug_assert!(self.homogeneity.matches(entity, world));
        // TODO attempt to combine identical entities, which will require killing the old copy

        // add distinct item
        self.contents.push(StackedEntity { entity, count: 1 });
        self.total_count += 1;
    }

    pub fn is_full(&self) -> bool {
        self.contents.len() >= self.contents.capacity()
    }

    /// current, limit
    pub fn capacity(&self) -> (u16, u16) {
        (self.total_count, self.contents.capacity() as u16)
    }

    pub fn contents(&self) -> impl Iterator<Item = (Entity, u16)> + '_ {
        self.contents.iter().map(|e| (e.entity, e.count))
    }

    pub fn total_count(&self) -> u16 {
        self.total_count
    }
}

impl StackHomogeneity {
    pub fn from_entity(e: Entity, world: &EcsWorld) -> Option<Self> {
        world
            .component::<DefinitionNameComponent>(e)
            .ok()
            .map(|def| StackHomogeneity {
                definition: def.0.clone(),
            })
    }

    pub fn matches(&self, e: Entity, world: &EcsWorld) -> bool {
        world
            .component::<DefinitionNameComponent>(e)
            .map(|def| self.definition == def.0)
            .unwrap_or(false)
    }
}

#[cfg(debug_assertions)]
mod validation {
    use super::*;
    use crate::item::stack::ItemStackComponent;
    use crate::item::HauledItemComponent;
    use crate::{ContainedInComponent, TransformComponent};
    use std::collections::HashMap;

    impl ItemStackComponent {
        /// Asserts all items dont have transforms, aren't duplicates, are alive, and that stacks
        /// are valid
        /// - held_entities: item->holder
        pub fn validate(
            &self,
            container: Entity,
            world: &impl ComponentWorld,
            held_entities: &mut HashMap<Entity, ContainedInComponent>,
        ) {
            validate_stack(&self.stack, container, held_entities, world);
        }
    }

    //noinspection DuplicatedCode
    fn validate_stack(
        stack: &ItemStack,
        stack_entity: Entity,
        held_entities: &mut HashMap<Entity, ContainedInComponent>,
        world: &impl ComponentWorld,
    ) {
        // validate count
        let real_count: u16 = stack.contents.iter().map(|e| e.count).sum();
        assert_eq!(real_count, stack.total_count, "stack count is wrong");

        assert!(
            !stack.contents.is_empty() && stack.total_count > 0,
            "stack is empty and should be collapsed"
        );

        for &StackedEntity { entity, .. } in &stack.contents {
            assert!(world.is_entity_alive(entity), "item {} is dead", entity);

            if let Some(other_holder) =
                held_entities.insert(entity, ContainedInComponent::StackOf(stack_entity))
            {
                let contained = world.component::<ContainedInComponent>(entity).ok();
                if let Some(contained) = contained {
                    // this item has already been visited in another inventory
                    let holder = contained.entity();
                    assert_eq!(
                        holder, stack_entity,
                        "item {} found in stack {} has invalid ContainedInComponent '{}'",
                        entity, stack_entity, *contained
                    );
                } else {
                    panic!(
                        "item {} is in the stack {} and also {}",
                        entity, stack_entity, other_holder,
                    );
                }
            }

            assert!(
                !world.has_component::<TransformComponent>(entity),
                "item {} in stack has a transform",
                entity
            );

            assert!(
                !world.has_component::<ItemStackComponent>(entity),
                "item {} in stack is a nested stack",
                entity
            );

            assert!(
                !world.has_component::<HauledItemComponent>(entity),
                "item {} in stack has a hauled component",
                entity
            );

            let contained = world
                .component::<ContainedInComponent>(entity)
                .unwrap_or_else(|_| {
                    panic!(
                        "item {} in stack does not have a contained component",
                        entity
                    )
                });

            let contained = contained.entity();
            assert_eq!(
                contained, stack_entity,
                "item {} in stack {} has a mismatching contained-in: {}",
                entity, stack_entity, contained,
            );
        }
    }
}
