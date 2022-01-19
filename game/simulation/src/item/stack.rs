use std::cmp::Ordering;
use std::collections::VecDeque;
use std::num::NonZeroU16;

use common::*;

use crate::definitions::DefinitionNameComponent;
use crate::ecs::*;

use crate::string::CachedStr;
use crate::PhysicalComponent;

#[derive(Debug, Error, Eq, PartialEq, Clone)]
pub enum ItemStackError<E: Debug + Display + Eq + Clone> {
    #[error("Item stack is full")]
    Full,

    #[error("{0} is not homogeneous with the rest of the stack")]
    NotHomogeneous(E),

    #[error("{0} is not stackable, missing stackable/physical/transform component")]
    NotStackable(E),

    #[error("Item stackability must be >0")]
    ZeroStackability,

    #[error("Item is already stacked or in a container")]
    AlreadyStacked,

    #[error("Cannot calculate homogeneity for {0}")]
    CantGetHomogeneity(E),

    #[error("Invalid number for split: wanted {wanted} but have {size}")]
    InvalidSplitCount { size: u16, wanted: u16 },

    #[error("Item stack is empty")]
    Empty,

    #[error("Item stack of {0} overflowed")]
    Overflow(E),
}

/// A homogeneous stack of items
#[derive(Component, EcsComponent)]
#[name("item-stack")]
#[storage(DenseVecStorage)]
#[clone(disallow)]
pub struct ItemStackComponent {
    pub stack: crate::item::ItemStack,
}

pub trait World {
    type Entity: Debug + Display + Copy + Eq;
    type Homogeneity: Clone;
    type Copyability: Copyability;

    fn homogeneity_for(&self, e: Self::Entity) -> Option<Self::Homogeneity>;
    fn is_homogeneous(&self, e: Self::Entity, homogeneity: &Self::Homogeneity) -> bool;

    /// For combining
    fn is_identical(&self, a: Self::Entity, b: Self::Entity) -> bool;
}

pub trait Copyability {
    fn is_copyable(&self) -> bool;

    /// If not copyable, returns the name of the component to blame
    fn not_copyable_component(&self) -> Option<&'static str>;
}

/// A stack of homogeneous entities that are still distinct but stacked together. Examples might
/// be a stack of arrows holds 10xsteel_arrows, 12xwood_arrows
#[derive(Debug)]
pub struct ItemStack<W: World> {
    contents: VecDeque<StackedEntity<W>>,
    /// Count of all items across all stacks of identical entities
    total_count: u16,
    max_count: NonZeroU16,
    homogeneity: W::Homogeneity,
}

/// Defines the criteria for allowing an item into a stack
#[derive(Debug)]
pub struct StackHomogeneity<W: World> {
    // TODO use a better way than hacky definition names
    definition: CachedStr,
    phantom: PhantomData<W>,
}

#[derive(Debug)] // TODO implement Debug manually
struct StackedEntity<W: World> {
    entity: W::Entity,
    count: StackedEntityCount,
}

#[derive(Debug, Copy)]
enum StackedEntityCount {
    /// Non-copyable entity
    Distinct,
    Copyable(NonZeroU16),
}

pub enum StackAdd {
    /// Item was added as distinct item
    Distinct,
    /// Item was collapsed into another identical stack
    CollapsedIntoOther,
}

#[derive(Copy)]
pub struct StackMigrationOp<W: World> {
    pub item: W::Entity,
    pub ty: StackMigrationType,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StackMigrationType {
    MoveDistinct,
    Move(NonZeroU16),
    Copy(NonZeroU16),
}

const ONE: NonZeroU16 = unsafe { NonZeroU16::new_unchecked(1) };

impl<W: World> Clone for StackMigrationOp<W> {
    fn clone(&self) -> Self {
        Self {
            item: self.item,
            ty: self.ty,
        }
    }
}

impl<W: World> PartialEq for StackMigrationOp<W> {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item && self.ty == other.ty
    }
}

impl<W: World> Eq for StackMigrationOp<W> {}

impl<W: World> ItemStack<W> {
    pub fn new_with_item(
        max_size: NonZeroU16,
        first_item: W::Entity,
        copyability: W::Copyability,
        world: &W,
    ) -> Result<Self, ItemStackError<W::Entity>> {
        let homogeneity = world
            .homogeneity_for(first_item)
            .ok_or(ItemStackError::CantGetHomogeneity(first_item))?;

        let mut stack = ItemStack {
            contents: VecDeque::with_capacity(max_size.get() as usize),
            total_count: 0,
            max_count: max_size,
            homogeneity,
        };

        let add = stack.add_internal(first_item, copyability, world)?;
        assert!(matches!(add, StackAdd::Distinct));
        Ok(stack)
    }

    fn empty_from_other(other: &Self) -> Self {
        Self {
            contents: VecDeque::with_capacity(other.contents.capacity()),
            total_count: 0,
            max_count: other.max_count,
            homogeneity: other.homogeneity.clone(),
        }
    }

    pub fn try_add(
        &mut self,
        entity: W::Entity,
        copyability: W::Copyability,
        world: &W,
    ) -> Result<StackAdd, ItemStackError<W::Entity>> {
        if self.is_full() {
            Err(ItemStackError::Full)
        } else if !world.is_homogeneous(entity, &self.homogeneity) {
            Err(ItemStackError::NotHomogeneous(entity))
        } else {
            self.add_internal(entity, copyability, world)
        }
    }

    /// Capacity and homogeneity must have been checked. Only fails on overflow
    fn add_internal(
        &mut self,
        entity: W::Entity,
        copyability: W::Copyability,
        world: &W,
    ) -> Result<StackAdd, ItemStackError<W::Entity>> {
        debug_assert!(!self.is_full());
        debug_assert!(world.is_homogeneous(entity, &self.homogeneity));

        let copyable = copyability.is_copyable();
        let collapse_into = if copyable {
            self.contents.iter_mut().find(|stacked| {
                matches!(stacked.count, StackedEntityCount::Copyable(_))
                    && world.is_identical(entity, stacked.entity)
            })
        } else {
            None
        };

        let add = if let Some(StackedEntity {
            count: StackedEntityCount::Copyable(count),
            entity: collapsed_into,
            ..
        }) = collapse_into
        {
            // combine with identical entity
            // TODO spill over to a new stack instead of failing
            *count = NonZeroU16::new(count.get() + 1)
                .ok_or(ItemStackError::Overflow(*collapsed_into))?;
            StackAdd::CollapsedIntoOther
        } else {
            // add distinct item
            self.contents.push_back(StackedEntity {
                entity,
                count: if copyable {
                    StackedEntityCount::Copyable(ONE)
                } else {
                    StackedEntityCount::Distinct
                },
            });

            StackAdd::Distinct
        };

        self.total_count += 1;
        Ok(add)
    }

    /// Returns Ok(None) if n is the exact size of the stack
    pub fn split_off<A: smallvec::Array<Item = StackMigrationOp<W>>>(
        &mut self,
        n: NonZeroU16,
        ops_out: &mut SmallVec<A>,
    ) -> Result<Option<ItemStack<W>>, ItemStackError<W::Entity>> {
        // TODO provide an ItemFilter to split a specific range?

        match n.get().cmp(&self.total_count) {
            Ordering::Greater => {
                return Err(ItemStackError::InvalidSplitCount {
                    wanted: n.get(),
                    size: self.total_count,
                })
            }
            Ordering::Equal => return Ok(None),
            _ => {}
        }

        debug_assert!(ops_out.is_empty());

        let mut remaining = n.get();
        let mut final_copy = None;
        for stacked in self.contents.iter_mut() {
            if remaining == 0 {
                break;
            } else {
                let op = match &mut stacked.count {
                    StackedEntityCount::Distinct => {
                        // move single item
                        remaining -= 1;
                        StackMigrationType::MoveDistinct
                    }
                    StackedEntityCount::Copyable(n) if remaining < n.get() => {
                        // remove part of the stack
                        *n = NonZeroU16::new(n.get() - remaining).unwrap(); // checked already
                        let to_take = NonZeroU16::new(remaining).unwrap(); // checked already
                        final_copy = Some(StackedEntity {
                            entity: stacked.entity,
                            count: StackedEntityCount::Copyable(to_take),
                        });
                        remaining = 0;
                        StackMigrationType::Copy(to_take)
                    }
                    StackedEntityCount::Copyable(n) => {
                        // move whole stack
                        debug_assert!(remaining >= n.get());
                        remaining -= n.get();
                        StackMigrationType::Move(*n)
                    }
                };

                ops_out.push(StackMigrationOp {
                    item: stacked.entity,
                    ty: op,
                });
            }
        }

        let move_count = if final_copy.is_some() {
            ops_out.len() - 1
        } else {
            ops_out.len()
        };

        // make new empty stack
        let mut new_stack = ItemStack::empty_from_other(self);

        for popped in self
            .contents
            .drain(..move_count)
            .chain(final_copy.into_iter())
        {
            debug_assert!(ops_out.iter().any(|op| op.item == popped.entity), "bad pop");

            new_stack.contents.push_back(popped);
        }

        new_stack.total_count = n.get();
        self.total_count -= n.get();

        Ok(Some(new_stack))
    }

    pub fn replace_entity(&mut self, orig: W::Entity, replacement: W::Entity) -> bool {
        if let Some(stacked) = self
            .contents
            .iter_mut()
            .find(|stacked| stacked.entity == orig)
        {
            stacked.entity = replacement;
            true
        } else {
            false
        }
    }

    pub fn is_full(&self) -> bool {
        self.total_count >= self.max_count.get()
    }

    /// current, limit
    pub fn filled(&self) -> (u16, u16) {
        (self.total_count, self.capacity().get())
    }

    pub fn contents(&self) -> impl Iterator<Item = (W::Entity, NonZeroU16)> + '_ {
        self.contents.iter().map(|e| (e.entity, e.count()))
    }

    pub fn total_count(&self) -> u16 {
        self.total_count
    }

    fn capacity(&self) -> NonZeroU16 {
        self.max_count
    }
}

impl<W: World> Clone for StackHomogeneity<W> {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition,
            phantom: PhantomData,
        }
    }
}

impl Clone for StackedEntityCount {
    fn clone(&self) -> Self {
        match self {
            StackedEntityCount::Distinct => StackedEntityCount::Distinct,
            StackedEntityCount::Copyable(n) => StackedEntityCount::Copyable(*n),
        }
    }
}

impl<W: World> Clone for StackedEntity<W> {
    fn clone(&self) -> Self {
        Self {
            entity: self.entity,
            count: self.count,
        }
    }
}

impl<W: World> StackedEntity<W> {
    fn count(&self) -> NonZeroU16 {
        match self.count {
            StackedEntityCount::Distinct => ONE,
            StackedEntityCount::Copyable(n) => n,
        }
    }
}

pub struct EntityCopyability {
    non_copyable_component: Option<&'static str>,
}

impl EntityCopyability {
    pub fn for_entity(world: &EcsWorld, entity: Entity) -> Self {
        Self {
            non_copyable_component: world.find_non_copyable(entity),
        }
    }
}

impl Copyability for EntityCopyability {
    fn is_copyable(&self) -> bool {
        self.non_copyable_component.is_none()
    }

    fn not_copyable_component(&self) -> Option<&'static str> {
        self.non_copyable_component
    }
}

impl World for EcsWorld {
    type Entity = crate::Entity;
    type Homogeneity = StackHomogeneity<Self>;
    type Copyability = EntityCopyability;

    fn homogeneity_for(&self, e: Self::Entity) -> Option<Self::Homogeneity> {
        self.component::<DefinitionNameComponent>(e)
            .ok()
            .map(|comp| StackHomogeneity {
                definition: comp.0,
                phantom: PhantomData,
            })
    }

    fn is_homogeneous(&self, e: Self::Entity, homogeneity: &Self::Homogeneity) -> bool {
        self.component::<DefinitionNameComponent>(e)
            .map(|def| homogeneity.definition == def.0)
            .unwrap_or(false)
    }

    fn is_identical(&self, _a: Self::Entity, _b: Self::Entity) -> bool {
        // TODO compare components
        false
    }
}

#[cfg(debug_assertions)]
mod validation {
    use std::collections::HashMap;

    use crate::item::HauledItemComponent;
    use crate::{ComponentWorld, ContainedInComponent, Entity, ItemStack, TransformComponent};

    use super::*;

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
        trace!("validating stack: {:?}", stack.contents().collect_vec());

        // validate count
        let real_count: u16 = stack.contents.iter().map(|e| e.count().get()).sum();
        assert_eq!(real_count, stack.total_count, "stack count is wrong");

        assert!(
            !stack.contents.is_empty() && stack.total_count > 0,
            "stack is empty and should be collapsed"
        );

        // validate volume
        let real_volume = world
            .component::<PhysicalComponent>(stack_entity)
            .expect("stack missing physical component")
            .volume
            .get();

        let calc_volume = stack.contents().fold(0, |acc, (e, count)| {
            let vol = world
                .component::<PhysicalComponent>(e)
                .expect("item in stack missing physical component")
                .volume
                .get();
            acc + (vol * count.get())
        });

        assert_eq!(
            real_volume, calc_volume,
            "stack volume is wrong, should be {} but is {}",
            calc_volume, real_volume
        );

        for stacked_entity in &stack.contents {
            let entity = stacked_entity.entity;

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
                    )
                } else {
                    panic!(
                        "item {} is in the stack {} and also {}",
                        entity, stack_entity, other_holder,
                    )
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

impl<W: World> Debug for StackMigrationOp<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.ty {
            StackMigrationType::MoveDistinct => write!(f, "move {}", self.item),
            StackMigrationType::Move(n) => write!(f, "move {}x{}", n, self.item),
            StackMigrationType::Copy(n) => write!(f, "copy {}x{}", n, self.item),
        }
    }
}

#[cfg(test)]
mod tests {
    use StackMigrationType::*;

    use super::*;

    // items are u32s.
    // 2 classes of stacks: even and odd (homogeneity).
    // items are identical if they're equal % 100, e.g. 1 == 101 == 102
    // items % 100 >= 50 are NOT copyable i.e. they are distinct items.

    #[derive(Default)]
    struct TestWorld;

    impl Copyability for bool {
        fn is_copyable(&self) -> bool {
            *self
        }

        fn not_copyable_component(&self) -> Option<&'static str> {
            unreachable!()
        }
    }

    fn is_copyable(item: u32) -> bool {
        (item % 100) < 50
    }

    impl World for TestWorld {
        type Entity = u32;
        /// even=true, odd=false
        type Homogeneity = bool;
        type Copyability = bool;

        fn homogeneity_for(&self, e: Self::Entity) -> Option<Self::Homogeneity> {
            Some(e % 2 == 0)
        }

        fn is_homogeneous(&self, e: Self::Entity, homogeneity: &Self::Homogeneity) -> bool {
            let this = self.homogeneity_for(e).unwrap();
            this == *homogeneity
        }

        fn is_identical(&self, a: Self::Entity, b: Self::Entity) -> bool {
            (a % 100) == (b % 100)
        }
    }

    struct TestStack {
        world: TestWorld,
        stack: ItemStack<TestWorld>,
    }

    impl TestStack {
        fn new(first_item: u32, cap: u16) -> Self {
            let world = TestWorld::default();
            let stack = ItemStack::new_with_item(
                NonZeroU16::new(cap).unwrap(),
                first_item,
                is_copyable(first_item),
                &world,
            )
            .expect("new stack");

            Self { world, stack }
        }

        fn new_with(items: &[u32]) -> Self {
            let mut items = items.iter().copied();
            let mut stack = Self::new(items.next().expect("no items"), 10);
            for item in items {
                stack.add(item).expect("failed to add");
            }

            stack
        }

        fn add(&mut self, item: u32) -> Result<StackAdd, ItemStackError<u32>> {
            self.stack.try_add(item, is_copyable(item), &self.world)
        }

        fn contents(&self) -> Vec<(u32, u16)> {
            self.stack
                .contents()
                .map(|(e, n)| (e, n.get()))
                .collect_vec()
        }

        fn split_off_full(
            &mut self,
            n: u16,
        ) -> Result<
            (
                Option<ItemStack<TestWorld>>,
                Vec<StackMigrationOp<TestWorld>>,
            ),
            ItemStackError<u32>,
        > {
            let mut ops = SmallVec::<[_; 1]>::new();
            self.stack
                .split_off(NonZeroU16::new(n).expect("bad split count"), &mut ops)
                .map(|stack| (stack, ops.into_vec()))
        }

        fn split_off(
            &mut self,
            n: u16,
        ) -> Result<Vec<(u32, StackMigrationType)>, ItemStackError<u32>> {
            self.split_off_full(n)
                .map(|(_, ops)| ops.into_iter().map(|op| (op.item, op.ty)).collect_vec())
        }
    }

    #[test]
    fn full_with_distinct() {
        let mut stack = TestStack::new(1, 4);

        assert!(matches!(stack.add(3), Ok(StackAdd::Distinct)));
        assert!(matches!(stack.add(5), Ok(StackAdd::Distinct)));
        assert!(matches!(stack.add(7), Ok(StackAdd::Distinct)));

        assert!(stack.stack.is_full());
        assert!(matches!(stack.add(9), Err(ItemStackError::Full)));
    }

    #[test]
    fn combine_uniques() {
        let mut stack = TestStack::new(1, 4);

        assert!(matches!(stack.add(101), Ok(StackAdd::CollapsedIntoOther)));
        assert!(matches!(stack.add(201), Ok(StackAdd::CollapsedIntoOther)));
        assert!(matches!(stack.add(3), Ok(StackAdd::Distinct)));

        assert_eq!(stack.contents(), vec![(1, 3), (3, 1)]);

        assert!(stack.stack.is_full());
        assert!(matches!(stack.add(3), Err(ItemStackError::Full)));
        assert!(matches!(stack.add(9), Err(ItemStackError::Full)));
    }

    #[test]
    fn homogeneity() {
        let mut odd_stack = TestStack::new(1, 10);
        assert!(matches!(
            odd_stack.add(2),
            Err(ItemStackError::NotHomogeneous(_))
        ));

        let mut even_stack = TestStack::new(2, 10);
        assert!(matches!(
            even_stack.add(5),
            Err(ItemStackError::NotHomogeneous(_))
        ));
    }

    #[test]
    fn split_whole_stack() {
        let mut stack = TestStack::new_with(&[1, 3, 5]);
        let (new_stack, ops) = stack.split_off_full(3).expect("failed");
        assert!(new_stack.is_none());
        assert!(ops.is_empty());
    }

    #[test]
    fn split_distinct() {
        let mut stack = TestStack::new_with(&[51, 3, 5]);
        assert_eq!(stack.contents(), vec![(51, 1), (3, 1), (5, 1)]);

        assert_eq!(stack.split_off(1), Ok(vec![(51, MoveDistinct)]));
        assert_eq!(stack.contents(), vec![(3, 1), (5, 1)]);

        assert_eq!(stack.split_off(1), Ok(vec![(3, Move(ONE))]));

        assert!(stack.split_off(5).is_err());
    }

    #[test]
    fn split_combined() {
        let mut stack = TestStack::new_with(&[2, 4, 6, 202, 302, 304]);
        assert_eq!(stack.contents(), vec![(2, 3), (4, 2), (6, 1)]);

        assert_eq!(
            stack.split_off(2),
            Ok(vec![(2, Copy(NonZeroU16::new(2).unwrap()))])
        );
        assert_eq!(stack.contents(), vec![(2, 1), (4, 2), (6, 1)]);

        assert_eq!(
            stack.split_off(3),
            Ok(vec![(2, Move(ONE)), (4, Move(NonZeroU16::new(2).unwrap()))])
        );
        assert_eq!(stack.contents(), vec![(6, 1)]);
    }
}
