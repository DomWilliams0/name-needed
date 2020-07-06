use common::*;

use crate::ecs::*;
use crate::item::inventory::Contents;
use crate::item::{
    ContentsGetError, ContentsPutError, ItemFilter, ItemSlot, MountedInventoryIndex, SlotIndex,
};

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum SlotReference {
    Base(SlotIndex),
    Mounted(MountedInventoryIndex, SlotIndex),
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct ItemReference(pub SlotReference, pub Entity);

/// Same as an ItemReference but comparing for equality only uses the entity instead
/// of both entity and slot position
#[derive(Debug, Clone, Copy)]
pub struct LooseItemReference(pub ItemReference);

#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub enum InventoryError {
    SlotOutOfRange,
    InventoryOutOfRange(MountedInventoryIndex),
    ContentsGet(ContentsGetError),
    ContentsPut(ContentsPutError),
    EmptySlot,
    NoFreeMountedSlots,
    NoUsableBaseSlot,
}

pub enum BaseSlotPolicy {
    /// Always choose the dominant if its usable, even if the other(s) are empty e.g. for a tool
    AlwaysDominant,

    /// Never choose the dominant
    NeverDominant,

    /// Choose other non dominant if they're empty and dominant isn't e.g. just to hold something
    Any,

    /// Choose any empty slot that isn't the given one
    AnyEmptyExcept(SlotIndex),
}

#[derive(Component)]
#[storage(DenseVecStorage)]
#[cfg_attr(test, derive(Clone))]
pub struct InventoryComponent {
    /// Primary quick access slots e.g. in a human's hands, in a dog's mouth
    base: Contents,

    /// Index of dominant base slot e.g. human's right hand
    dominant_base: SlotIndex, // TODO option

    /// Secondary inventory slots e.g. a bag, a chest
    /// `Some`s don't have to be contiguous
    mounted: SmallVec<[Option<Contents>; 2]>,
}

impl InventoryComponent {
    pub fn new(
        base_inv_size: usize,
        mounted_inv_count: usize,
        dominant_base: Option<usize>,
    ) -> Self {
        if let Some(dominant) = dominant_base {
            assert!(dominant < base_inv_size);
        }

        // vec should never need to expand any more
        let mut mounted = SmallVec::with_capacity(mounted_inv_count);
        mounted.resize(mounted_inv_count, None);

        Self {
            base: Contents::with_size(base_inv_size),
            dominant_base: dominant_base.unwrap_or(0),
            mounted,
        }
    }

    /// **Asserts** all items are alive and actually items
    #[cfg(debug_assertions)]
    pub fn validate<W: ComponentWorld>(&self, world: &W) {
        self.base.validate_items(world);
        self.mounted
            .iter()
            .filter_map(|inv| inv.as_ref())
            .for_each(|inv| inv.validate_items(world));
    }

    #[must_use = "Searching is expensive!"]
    pub fn search<W: ComponentWorld>(
        &self,
        filter: &ItemFilter,
        world: &W,
    ) -> Option<ItemReference> {
        // TODO cache result of search until they change (specs::storage::Tracked?)

        if let Some((slot, entity)) = self.base.search(filter, Some(world)) {
            return Some(ItemReference(SlotReference::Base(slot), entity));
        }

        for (inv, contents) in self.mounted.iter().enumerate() {
            if let Some(contents) = contents {
                if let Some((slot, entity)) = contents.search(filter, Some(world)) {
                    return Some(ItemReference(SlotReference::Mounted(inv, slot), entity));
                }
            }
        }

        None
    }

    /// Gets usable (Free|Empty) base slot, using the given policy
    pub fn usable_base_slot(&self, policy: BaseSlotPolicy) -> Option<SlotIndex> {
        let dominant = self.base.get_slot_exact(self.dominant_base).ok()?;

        match (policy, dominant) {
            // dominant is usable, use that always
            (BaseSlotPolicy::AlwaysDominant, slot) if slot.usable() => Some(self.dominant_base),
            // dominant is empty, use that
            (BaseSlotPolicy::Any, ItemSlot::Empty) => Some(self.dominant_base),
            // find any EMPTY that isn't the given slot
            (BaseSlotPolicy::AnyEmptyExcept(except), _) => self
                .base
                .get_first_slot_with_index(|&(idx, slot)| idx != except && slot.empty()),
            // find any usable that isn't the dominant
            (BaseSlotPolicy::NeverDominant, _) => {
                self.base.get_first_slot_with_index(|&(idx, slot)| {
                    idx != self.dominant_base && slot.usable()
                })
            }
            // fallback to finding any empty
            _ => self.base.get_first_slot(ItemSlot::empty),
        }
    }

    /// Finds a usable base slot and frees it up by swapping the held item into mounted storage, if
    /// any. Preference in order: free slots, usable non-dominant slots, usable dominant slot
    fn free_up_base_slot(&mut self, size: SlotIndex) -> Option<SlotIndex> {
        // TODO free up base slots for items bigger than 1
        assert_eq!(size, 1, "not implemented for bigger items");

        // if theres a base slot free already, we're done
        if let Some(free) = self.base.get_first_slot(ItemSlot::empty) {
            return Some(free);
        }

        // find a free slot in a mounted inv
        let free_mounted_slot = (|| {
            for (idx, mounted) in self.mounted.iter().enumerate() {
                if let Some(slot) = mounted
                    .as_ref()
                    .and_then(|inv| inv.get_first_slot(ItemSlot::empty))
                {
                    return Some(SlotReference::Mounted(idx, slot));
                }
            }
            None
        })();

        if let Some(mounted_slot) = free_mounted_slot {
            // we have an empty slot in a mounted inv, choose a base slot to free up,
            // preferably non-dominant

            let dominant = self.dominant_base;

            let base_slot = self
                .usable_base_slot(BaseSlotPolicy::NeverDominant)
                .or_else(|| {
                    // fine, check dominant last
                    self.base.get_slot_exact(dominant).ok().and_then(|slot| {
                        if slot.usable() {
                            Some(dominant)
                        } else {
                            None
                        }
                    })
                });

            if let Some(base_slot) = base_slot {
                // nice, swap them
                return self
                    .swap(SlotReference::Base(base_slot), mounted_slot)
                    .map(|_| base_slot)
                    .ok();
            }
        }

        None
    }

    pub fn give_item(&mut self, item: Entity, base_size: SlotIndex) -> Result<(), InventoryError> {
        self.free_up_base_slot(base_size)
            .ok_or(InventoryError::NoUsableBaseSlot)
            .and_then(|slot| {
                self.base
                    .put_item(item, slot, base_size)
                    .map_err(InventoryError::ContentsPut)
            })
    }

    pub fn remove_item(&mut self, slot: SlotReference) -> Result<Entity, InventoryError> {
        match slot {
            SlotReference::Base(idx) => {
                self.base.remove_item(idx)?.ok_or(InventoryError::EmptySlot)
            }
            SlotReference::Mounted(inv, idx) => self
                .get_mounted_mut(inv)?
                .remove_item(idx)?
                .ok_or(InventoryError::EmptySlot),
        }
    }

    pub fn get(&self, slot: SlotReference) -> Result<Entity, InventoryError> {
        let item = match slot {
            SlotReference::Base(slot) => self
                .base
                .get_item(slot)
                .map_err(InventoryError::ContentsGet),
            SlotReference::Mounted(i, slot) => self
                .get_mounted(i)?
                .get_item(slot)
                .map_err(InventoryError::ContentsGet),
        }?;

        item.ok_or(InventoryError::EmptySlot)
    }

    fn get_mounted(&self, idx: MountedInventoryIndex) -> Result<&Contents, InventoryError> {
        self.mounted
            .get(idx)
            .and_then(|inv| inv.as_ref()) // has slot -> is slot full
            .ok_or(InventoryError::InventoryOutOfRange(idx))
    }

    fn get_mounted_mut(
        &mut self,
        idx: MountedInventoryIndex,
    ) -> Result<&mut Contents, InventoryError> {
        self.mounted
            .get_mut(idx)
            .and_then(|inv| inv.as_mut()) // has slot -> is slot full
            .ok_or(InventoryError::InventoryOutOfRange(idx))
    }

    fn get_contents_mut(
        &mut self,
        slot: SlotReference,
    ) -> Result<(&mut Contents, SlotIndex), InventoryError> {
        match slot {
            SlotReference::Base(slot) => Ok((&mut self.base, slot)),
            SlotReference::Mounted(inv, slot) => {
                self.get_mounted_mut(inv).map(|contents| (contents, slot))
            }
        }
    }

    fn swap(&mut self, a: SlotReference, b: SlotReference) -> Result<(), InventoryError> {
        if a == b {
            // nop
            Ok(())
        } else {
            // safety: a != b and don't overlap, asserted before use below
            let (a, a_slot): (&mut Contents, SlotIndex) =
                unsafe { std::mem::transmute(self.get_contents_mut(a)?) };
            let (b, b_slot) = self.get_contents_mut(b)?;

            // TODO swap items bigger than 1
            assert!(
                a.get_slot_exact(a_slot)
                    .map(|slot| slot.usable())
                    .unwrap_or(true),
                "swapping items bigger than 1 not yet implemented"
            );
            assert!(
                b.get_slot_exact(b_slot)
                    .map(|slot| slot.usable())
                    .unwrap_or(true),
                "swapping items bigger than 1 not yet implemented"
            );

            a.swap_with(a_slot, b, b_slot)
                .map_err(InventoryError::ContentsGet)
        }
    }

    // TODO add a component that allows accessing your mounted storage - animals can wear them but not use!

    //
    pub fn equip(
        &mut self,
        src: SlotReference,
        policy: BaseSlotPolicy,
    ) -> Result<SlotIndex, InventoryError> {
        // find base slot to move the item to
        let base_dst = self
            .usable_base_slot(policy)
            .ok_or(InventoryError::NoUsableBaseSlot)?;

        // copy original base slot that we're about to swap into
        let base_overridden = self.base.get_slot_exact(base_dst).unwrap(); // base_dst returned so must be valid

        // swap src item into base
        self.swap(SlotReference::Base(base_dst), src)?;

        // original base item is now in `src`: if it's not empty try to find another base slot to swap it to if possible
        if base_overridden.full() {
            if let Some(slot) = self.usable_base_slot(BaseSlotPolicy::AnyEmptyExcept(base_dst)) {
                // found a usable base slot that isn't the new one, lets use that
                self.swap(src, SlotReference::Base(slot))?;
            };
        }

        // TODO what if original item is bigger than 1?

        Ok(base_dst)
    }

    // Returns the contents back on error
    pub fn give_mounted(
        &mut self,
        contents: Contents,
    ) -> Result<MountedInventoryIndex, (InventoryError, Contents)> {
        // find first free mounted slot
        let available = match self.mounted.iter().position(|slot| slot.is_none()) {
            None => return Err((InventoryError::NoFreeMountedSlots, contents)),
            Some(i) => i,
        };

        // safety: index return by iterator above
        let mounted = unsafe { self.mounted.get_unchecked_mut(available) };
        let _ = mounted.replace(contents);
        Ok(available)
    }

    pub fn take_mounted(
        &mut self,
        index: MountedInventoryIndex,
    ) -> Result<Contents, InventoryError> {
        let mounted = self
            .mounted
            .get_mut(index)
            .ok_or_else(|| InventoryError::InventoryOutOfRange(index))?;
        match mounted {
            None => Err(InventoryError::EmptySlot),
            c @ Some(_) => Ok(c.take().unwrap()),
        }
    }
}

impl From<ContentsGetError> for InventoryError {
    fn from(e: ContentsGetError) -> Self {
        InventoryError::ContentsGet(e)
    }
}

impl Display for InventoryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:?}", self)
    }
}

impl Error for InventoryError {}

impl PartialEq for LooseItemReference {
    fn eq(&self, other: &Self) -> bool {
        // only compare entity
        (self.0).1 == (other.0).1
    }
}

impl Eq for LooseItemReference {}

#[cfg(test)]
mod tests {
    use matches::assert_matches;

    use crate::item::inventory::test::test_dummy_items;
    use crate::item::*;

    use super::*;

    #[test]
    fn equip() {
        let (_, [food, weapon, other_weapon]) = test_dummy_items();

        let dominant = 1;
        let non_dominant = 0;
        let mut inv = InventoryComponent::new(2, 2, Some(dominant));
        let mut bag = Contents::with_size(4);

        // holding weapon in dominant hand, food is in bag, which is mounted
        inv.base.put_item(weapon, dominant, 1).unwrap();
        bag.put_item(food, 2, 1).unwrap();
        inv.give_mounted(bag).unwrap();

        let food_pos = SlotReference::Mounted(0, 2);

        // move food from bag to any, putting it in non dominant. weapon is still in dominant
        assert_eq!(inv.equip(food_pos, BaseSlotPolicy::Any), Ok(non_dominant));
        assert_eq!(inv.get(SlotReference::Base(dominant)), Ok(weapon));

        // put food back in bag for next test
        inv.swap(SlotReference::Base(non_dominant), food_pos)
            .unwrap();

        // now move food from bag to dominant, which will push the weapon into the non dominant hand
        assert_eq!(
            inv.equip(food_pos, BaseSlotPolicy::AlwaysDominant),
            Ok(dominant)
        );
        assert_eq!(inv.get(SlotReference::Base(non_dominant)), Ok(weapon));

        // move them back for next test
        inv.swap(SlotReference::Base(dominant), food_pos).unwrap();
        inv.swap(
            SlotReference::Base(non_dominant),
            SlotReference::Base(dominant),
        )
        .unwrap();

        // now both hands are full, move food from bag to dominant and swap the dominant weapon to
        // the bag, leaving the non dominant weapon there
        inv.base.put_item(other_weapon, non_dominant, 1).unwrap();
        assert_eq!(
            inv.equip(food_pos, BaseSlotPolicy::AlwaysDominant),
            Ok(dominant)
        );
        assert_eq!(inv.get(SlotReference::Base(non_dominant)), Ok(other_weapon));
        assert_eq!(inv.get(food_pos), Ok(weapon));
    }

    #[test]
    fn mounted() {
        let (world, [food, weapon, _]) = test_dummy_items();
        let mut inv = InventoryComponent::new(2, 2, None);

        // weapon is in base
        inv.base.put_item(weapon, 1, 1).unwrap();

        // bag has food
        let mut bag = Contents::with_size(4);
        bag.put_item(food, 3, 1).unwrap();

        // no mounted
        assert_matches!(
            inv.get_mounted(0),
            Err(InventoryError::InventoryOutOfRange(0))
        );
        assert_matches!(
            inv.get_mounted(1),
            Err(InventoryError::InventoryOutOfRange(1))
        );
        assert_matches!(
            inv.get_mounted(2),
            Err(InventoryError::InventoryOutOfRange(2))
        );

        // give bag
        assert_matches!(inv.give_mounted(bag), Ok(0));
        let bag = inv.get_mounted(0).expect("bag has been given");

        // bag still has food in it, phew
        assert_eq!(bag.get_item(3), Ok(Some(food)));

        // give a tiny basket too
        let tiny_basket = Contents::with_size(1);
        assert_matches!(inv.give_mounted(tiny_basket), Ok(1));

        // now theres no more space for more
        let tiny_violin = Contents::with_size(1);
        assert_matches!(
            inv.give_mounted(tiny_violin),
            Err((InventoryError::NoFreeMountedSlots, _the_violin))
        );

        // weapon is found in base
        assert_matches!(
            inv.search(&ItemFilter::Class(ItemClass::Weapon), &world),
            Some(ItemReference(SlotReference::Base(1), _))
        );

        // food is found in bag
        assert_matches!(
            inv.search(&ItemFilter::SpecificEntity(food), &world),
            Some(ItemReference(SlotReference::Mounted(0, 3), _))
        );

        // take bag away - food is still in it
        let bag = inv.take_mounted(0).unwrap();
        assert_eq!(bag.get_item(3), Ok(Some(food)));

        // bag is gone
        assert_matches!(
            inv.get_mounted(0),
            Err(InventoryError::InventoryOutOfRange(0))
        );
        // basket remains
        assert_matches!(inv.get_mounted(1), Ok(_));
    }

    #[test]
    fn dominant_base() {
        let (_, [food, _, _]) = test_dummy_items();

        let do_test = |dominant, non_dominant| {
            // 2 hands
            let mut inv = InventoryComponent::new(2, 0, Some(dominant));

            // choose dominant when both are empty
            assert_eq!(
                inv.usable_base_slot(BaseSlotPolicy::AlwaysDominant),
                Some(dominant)
            );
            assert_eq!(inv.usable_base_slot(BaseSlotPolicy::Any), Some(dominant));

            // non dominant hand is full - still use dominant
            assert!(inv.base.put_item(food, non_dominant, 1).is_ok());
            assert_eq!(
                inv.usable_base_slot(BaseSlotPolicy::AlwaysDominant),
                Some(dominant)
            );
            assert_eq!(inv.usable_base_slot(BaseSlotPolicy::Any), Some(dominant));

            // dominant hand is full but other is free...
            assert!(inv.base.swap_internal(non_dominant, dominant).is_ok());

            // ...dominant is usable so choose that even though other hand is free
            assert_eq!(
                inv.usable_base_slot(BaseSlotPolicy::AlwaysDominant),
                Some(dominant)
            );

            // ...non dominant is free so use that
            assert_eq!(
                inv.usable_base_slot(BaseSlotPolicy::Any),
                Some(non_dominant)
            );

            // ...choose non dominant regardless
            assert_eq!(
                inv.usable_base_slot(BaseSlotPolicy::AnyEmptyExcept(dominant)),
                Some(non_dominant)
            );
            // ...no other usable base slots so this fails (dominant=full, non dominant=except)
            assert_eq!(
                inv.usable_base_slot(BaseSlotPolicy::AnyEmptyExcept(non_dominant)),
                None,
            );
        };

        do_test(0, 1);
        do_test(1, 0);
    }

    #[test]
    fn free_up_slot_one_hand() {
        let (_, [food, weapon, _]) = test_dummy_items();

        // 1 hands, no bags
        let mut inv = InventoryComponent::new(1, 1, None);

        // ..hand is free
        assert_eq!(inv.free_up_base_slot(1), Some(0));

        // fill up single hand - no slots free anymore
        inv.base.put_item(food, 0, 1).unwrap();
        assert!(inv.free_up_base_slot(1).is_none());

        // mount a tiny bag
        let bag = Contents::with_size(1);
        inv.give_mounted(bag).unwrap();

        // now the food will be moved to the bag so the hand is now free
        assert_eq!(inv.free_up_base_slot(1), Some(0));
        assert_matches!(
            inv.get(SlotReference::Base(0)),
            Err(InventoryError::EmptySlot)
        );
        assert_eq!(inv.get(SlotReference::Mounted(0, 0)), Ok(food));

        // now fill up his hand with the weapon - there are no free slots again
        inv.base.put_item(weapon, 0, 1).unwrap();
        assert!(inv.free_up_base_slot(1).is_none());
    }

    #[test]
    fn free_up_slot_two_hands() {
        let (_, [food, weapon, other_weapon]) = test_dummy_items();

        // 2 hands with 1 dominant
        let mut inv = InventoryComponent::new(2, 1, Some(1));

        // fill up non dominant hand, dominant is still free
        inv.base.put_item(food, 0, 1).unwrap();
        assert_eq!(inv.free_up_base_slot(1), Some(1));
        assert_eq!(
            inv.get(SlotReference::Base(1)),
            Err(InventoryError::EmptySlot)
        );

        // swap food to dominant hand, non dominant should be chosen now
        inv.base.swap_internal(0, 1).unwrap();
        assert_eq!(inv.free_up_base_slot(1), Some(0));
        assert_eq!(
            inv.get(SlotReference::Base(0)),
            Err(InventoryError::EmptySlot)
        );
        assert_eq!(inv.get(SlotReference::Base(1)), Ok(food));

        // mount a small bag
        let bag = Contents::with_size(2);
        inv.give_mounted(bag).unwrap();

        // fill up the second hand too, now the non-dominant hand should be swapped out
        inv.base.put_item(weapon, 0, 1).unwrap();
        assert_eq!(inv.free_up_base_slot(1), Some(0));
        assert_eq!(
            inv.get(SlotReference::Base(0)),
            Err(InventoryError::EmptySlot)
        );

        // fill it up again, spilling over to fill up the tiny bag
        inv.base.put_item(other_weapon, 0, 1).unwrap();
        assert_eq!(inv.free_up_base_slot(1), Some(0));
        assert!(inv
            .get_mounted(0)
            .unwrap()
            .get_first_slot(ItemSlot::empty)
            .is_none());
    }
}
