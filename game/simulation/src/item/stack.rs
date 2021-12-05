use crate::ecs::*;
use common::*;

#[derive(Debug, Error)]
pub enum ItemStackError {
    #[error("Item stack is full")]
    Full,
}

#[derive(Debug)]
pub struct ItemStack {
    contents: Vec<StackedEntity>,
    total_count: u16,
}

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
    pub fn new(max_size: u16) -> Self {
        ItemStack {
            contents: Vec::with_capacity(max_size as usize),
            total_count: 0,
        }
    }

    /// Does not touch any components, ensure they are updated
    pub fn try_add(&mut self, entity: Entity) -> Result<(), ItemStackError> {
        if self.is_full() {
            Err(ItemStackError::Full)
        } else {
            // TODO attempt to combine identical entities

            // add distinct item
            self.contents.push(StackedEntity { entity, count: 1 });
            self.total_count += 1;

            Ok(())
        }
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

        for &StackedEntity { entity, count } in &stack.contents {
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
