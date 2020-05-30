use std::fmt::Debug;
use std::hint::unreachable_unchecked;

use smallvec::alloc::fmt::Formatter;

use crate::ecs::{ComponentWorld, EcsWorld, Entity};
use crate::item::{BaseItemComponent, ItemFilter, ItemFilterable, SlotIndex};

/// [ empty ] [ full (small item 1 slot) ] [ empty ]
///
/// [ full (large item 3 slots) ] [ overflowed ] [ overflowed ] [ empty ]
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub enum ItemSlot {
    Empty,

    /// Entity ID of item - this is the first slot that the item takes up
    Full(Entity),

    /// Slot is full with overflow of a big item
    Overflow(SlotIndex),
    // TODO item slot disabled by (lack of) physical wellbeing e.g. missing hand
    // OutOfAction,
}

#[derive(Clone)]
pub struct Contents {
    // TODO can this be on the stack?
    slots: Box<[ItemSlot]>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum ContentsGetError {
    SlotOutOfRange,
    BadOverflowState(SlotIndex),
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum ContentsPutError {
    SlotOutOfRange,
    NotEnoughSpace,
    AlreadyContains,
}

impl Default for ItemSlot {
    fn default() -> Self {
        ItemSlot::Empty
    }
}

impl Contents {
    pub fn with_size(size: usize) -> Self {
        let slots = vec![ItemSlot::default(); size].into_boxed_slice();
        Self { slots }
    }

    pub fn size(&self) -> usize {
        self.slots.len()
    }

    pub fn search<W: ComponentWorld>(
        &self,
        filter: &ItemFilter,
        world: Option<&W>,
    ) -> Option<(SlotIndex, Entity)> {
        self.slots.iter().enumerate().find_map(|(i, slot)| {
            let filterable = (slot, world);
            filterable.matches(*filter).map(|e| (i, e))
        })
    }

    pub fn get_item(&self, index: SlotIndex) -> Result<Option<Entity>, ContentsGetError> {
        self.get_item_slot_index(index).map(|i| {
            // safety: returned index is verified to be Full(_)
            unsafe {
                match self.slots.get_unchecked(i) {
                    ItemSlot::Full(e) => Some(*e),
                    ItemSlot::Empty => None,
                    _ => unreachable_unchecked(),
                }
            }
        })
    }

    // Full|Empty
    pub fn get_item_slot_mut(
        &mut self,
        index: SlotIndex,
    ) -> Result<&mut ItemSlot, ContentsGetError> {
        self.get_item_slot_index(index).map(move |i| {
            // safety: returned index is verified to be Full|Empty
            unsafe { self.slots.get_unchecked_mut(i) }
        })
    }

    /// Ok(verified index of ItemSlot::Full|Empty)
    pub fn get_item_slot_index(&self, index: SlotIndex) -> Result<SlotIndex, ContentsGetError> {
        match self.get_slot_exact(index)? {
            ItemSlot::Empty | ItemSlot::Full(_) => Ok(index),
            ItemSlot::Overflow(idx) => {
                // get source slot item
                match self.slots.get(idx) {
                    Some(ItemSlot::Full(_)) => Ok(idx),
                    _ => Err(ContentsGetError::BadOverflowState(index)),
                }
            }
        }
    }

    /// Gets the given ItemSlot without resolving overflows
    pub fn get_slot_exact(&self, index: SlotIndex) -> Result<ItemSlot, ContentsGetError> {
        self.slots
            .get(index)
            .cloned()
            .ok_or(ContentsGetError::SlotOutOfRange)
    }

    fn get_slot_exact_mut(&mut self, index: SlotIndex) -> Result<&mut ItemSlot, ContentsGetError> {
        self.slots
            .get_mut(index)
            .ok_or(ContentsGetError::SlotOutOfRange)
    }

    pub fn get_first_slot<F: Fn(&ItemSlot) -> bool>(&self, pred: F) -> Option<SlotIndex> {
        self.slots.iter().position(pred)
    }

    pub fn get_first_slot_with_index<F: Fn(&(SlotIndex, &ItemSlot)) -> bool>(
        &self,
        pred: F,
    ) -> Option<SlotIndex> {
        self.slots.iter().enumerate().find(pred).map(|(i, _)| i)
    }

    pub fn get_first_usable_slot(&self) -> Option<SlotIndex> {
        self.get_first_slot(|slot| slot.usable())
    }

    /// self and other are different
    pub fn swap_with(
        &mut self,
        this_slot: SlotIndex,
        other: &mut Self,
        other_slot: SlotIndex,
    ) -> Result<(), ContentsGetError> {
        let src = self.get_slot_exact_mut(this_slot)?;
        let dst = other.get_slot_exact_mut(other_slot)?;

        Self::swap(src, dst)
    }

    pub fn swap_internal(&mut self, a: SlotIndex, b: SlotIndex) -> Result<(), ContentsGetError> {
        if a == b {
            // nop if same index
            return Ok(());
        }

        // safety: slots are asserted different from check above
        let a = unsafe { std::mem::transmute(self.get_slot_exact_mut(a)?) };
        let b = self.get_slot_exact_mut(b)?;

        Self::swap(a, b)
    }

    fn swap(a: &mut ItemSlot, b: &mut ItemSlot) -> Result<(), ContentsGetError> {
        // TODO handle different item sizes
        assert!(a.usable() && b.usable(), "not implemented");

        // swap a doodle doo
        std::mem::swap(a, b);
        Ok(())
    }

    pub fn put_item(
        &mut self,
        item: Entity,
        slot: SlotIndex,
        size: SlotIndex,
    ) -> Result<(), ContentsPutError> {
        // check this item is not already in here
        if cfg!(debug_assertions)
            && self
                .search(&ItemFilter::SpecificEntity(item), Option::<&EcsWorld>::None)
                .is_some()
        {
            return Err(ContentsPutError::AlreadyContains);
        }

        let slots = self
            .slots
            .get_mut(slot..slot + size)
            .ok_or(ContentsPutError::SlotOutOfRange)?;

        // check all slots are empty
        if slots.iter().any(|slot| !slot.empty()) {
            return Err(ContentsPutError::NotEnoughSpace);
        }

        // fill er up
        slots[0] = ItemSlot::Full(item);
        slots
            .iter_mut()
            .skip(1)
            .for_each(|overflow| *overflow = ItemSlot::Overflow(slot));
        Ok(())
    }

    /// Ok(Some(e)) -> slot was full
    /// Ok(None) -> slot was anything but full
    pub fn remove_item(&mut self, slot: SlotIndex) -> Result<Option<Entity>, ContentsGetError> {
        let slot = self.get_item_slot_mut(slot)?;
        let item = std::mem::take(slot);
        Ok(match item {
            ItemSlot::Full(e) => Some(e),
            _ => None,
        })
    }

    pub(crate) fn validate_items<W: ComponentWorld>(&self, world: &W) {
        assert!(
            self.slots
                .iter()
                .filter_map(|slot| match slot {
                    ItemSlot::Full(e) => Some(e),
                    _ => None,
                })
                .all(|item| {
                    // check items are alive and have base item component
                    world.component::<BaseItemComponent>(*item).is_ok()
                }),
            "inventory is holding invalid items: {:?}",
            self
        )
    }
}

impl ItemSlot {
    // Full|Empty
    pub fn usable(&self) -> bool {
        matches!(self, ItemSlot::Empty | ItemSlot::Full(_))
    }
    pub fn empty(&self) -> bool {
        matches!(self, ItemSlot::Empty)
    }
    pub fn full(&self) -> bool {
        matches!(self, ItemSlot::Full(_))
    }
}

impl Debug for Contents {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Contents({} slots, ", self.size())?;
        f.debug_list().entries(self.slots.iter()).finish()?;
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;

    use crate::item::inventory::contents::ItemSlot::Overflow;
    use crate::item::inventory::test::test_dummy_items;
    use crate::item::*;

    use super::*;

    #[test]
    fn add() {
        let (_, [food, weapon, _]) = test_dummy_items();
        let mut inv = Contents::with_size(6);

        // all initially empty
        assert!(inv.slots.iter().all(|s| s.empty()));

        // put small weapon in the middle successfully
        assert!(inv.put_item(weapon, 3, 1).is_ok());

        // can't put big item in near end
        assert_matches!(
            inv.put_item(food, 4, 5),
            Err(ContentsPutError::SlotOutOfRange)
        );

        // can't put big item at the beginning because the weapon is blocking it
        assert_matches!(
            inv.put_item(food, 0, 5),
            Err(ContentsPutError::NotEnoughSpace)
        );

        // move annoying weapon to the end
        assert!(inv.swap_internal(3, 5).is_ok());

        // now the big item can fit at the start
        assert!(inv.put_item(food, 0, 5).is_ok());

        assert_eq!(
            *inv.slots,
            [
                ItemSlot::Full(food),
                ItemSlot::Overflow(0),
                ItemSlot::Overflow(0),
                ItemSlot::Overflow(0),
                ItemSlot::Overflow(0),
                ItemSlot::Full(weapon),
            ]
        );
    }

    #[test]
    fn duplicate_add() {
        let (_, [food, weapon, _]) = test_dummy_items();
        let mut inv = Contents::with_size(6);

        assert!(inv.put_item(food, 0, 1).is_ok());
        assert!(inv.put_item(weapon, 1, 1).is_ok());
        assert_matches!(
            inv.put_item(food, 2, 1),
            Err(ContentsPutError::AlreadyContains)
        );
    }

    #[test]
    fn search() {
        let mut inv = Contents::with_size(2);
        let (world, [food, weapon, other_weapon]) = test_dummy_items();

        // empty
        assert!(inv
            .search(&ItemFilter::Class(ItemClass::Food), Some(&world))
            .is_none());
        assert!(inv
            .search(&ItemFilter::Class(ItemClass::Weapon), Some(&world))
            .is_none());
        assert!(inv
            .search(&ItemFilter::SpecificEntity(food), Some(&world))
            .is_none());

        // add food to  inventory
        assert_matches!(inv.put_item(food, 0, 1), Ok(()));

        assert_eq!(
            inv.search(&ItemFilter::Class(ItemClass::Food), Some(&world)),
            Some((0, food))
        );
        assert_eq!(
            inv.search(&ItemFilter::SpecificEntity(food), Some(&world)),
            Some((0, food))
        );
        assert!(inv
            .search(&ItemFilter::Class(ItemClass::Weapon), Some(&world))
            .is_none());

        // add weapon
        assert_matches!(inv.put_item(weapon, 1, 1), Ok(()));

        assert_eq!(
            inv.search(&ItemFilter::Class(ItemClass::Food), Some(&world)),
            Some((0, food)) // still there
        );
        assert_eq!(
            inv.search(&ItemFilter::Class(ItemClass::Weapon), Some(&world)),
            Some((1, weapon))
        );

        // no more spaces
        assert_matches!(
            inv.put_item(other_weapon, 2, 1),
            Err(ContentsPutError::SlotOutOfRange)
        );
    }

    #[test]
    fn impossibly_small() {
        let inv = Contents::with_size(0);
        assert!(inv.get_first_usable_slot().is_none());
    }

    #[test]
    fn swap_and_remove() {
        let (_, [food, weapon, _]) = test_dummy_items();

        let mut inv_a = Contents::with_size(2);
        assert!(inv_a.put_item(food, 0, 1).is_ok());

        assert_eq!(*inv_a.slots, [ItemSlot::Full(food), ItemSlot::Empty,]);

        assert_matches!(
            inv_a.swap_internal(0, 5),
            Err(ContentsGetError::SlotOutOfRange)
        );

        // swap with empty
        assert!(inv_a.swap_internal(0, 1).is_ok());
        assert_eq!(*inv_a.slots, [ItemSlot::Empty, ItemSlot::Full(food),]);

        // swap with item
        assert!(inv_a.put_item(weapon, 0, 1).is_ok());
        assert!(inv_a.swap_internal(0, 1).is_ok());
        assert_eq!(
            *inv_a.slots,
            [ItemSlot::Full(food), ItemSlot::Full(weapon),]
        );

        let mut inv_b = Contents::with_size(2);

        // swap food into other inv
        assert!(inv_a.swap_with(0, &mut inv_b, 1).is_ok());
        assert_eq!(*inv_a.slots, [ItemSlot::Empty, ItemSlot::Full(weapon),]);
        assert_eq!(*inv_b.slots, [ItemSlot::Empty, ItemSlot::Full(food),]);

        // remove
        assert_matches!(inv_a.remove_item(1), Ok(Some(_)));
        assert_matches!(inv_a.remove_item(1), Ok(None)); // is now empty
    }

    #[test]
    fn invalid_overflow_state() {
        let mut inv = Contents::with_size(10);

        // slot 3 is apparently filled by whats in slot 2
        *inv.get_item_slot_mut(3).unwrap() = Overflow(2);

        // 3 is the slot with the bad overflow
        assert_matches!(inv.get_item(3), Err(ContentsGetError::BadOverflowState(3)));
    }
}
