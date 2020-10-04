use std::collections::HashMap;
use std::hint::unreachable_unchecked;
use std::iter::repeat_with;
use std::ops::Range;

use common::*;
use unit::length::Length3;
use unit::volume::Volume;

use crate::ecs::*;
use crate::item::inventory2::equip::EquipSlot;
use crate::item::inventory2::{Container, HeldEntity};
use crate::item::{ItemFilter, ItemFilterable};
use crate::TransformComponent;

/// Temporary dumb component to hold equip slots and containers. Will eventually be a view on top of
/// the physical body tree
#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("inventory")]
pub struct Inventory2Component {
    equip_slots: Vec<EquipSlot>,
    containers: Vec<Container>,
}

// TODO debug inventory validation

#[derive(Debug)]
pub struct InventoryComponentTemplate {
    equip_slots: usize,
}

pub struct EquipSlots<'a>(&'a mut [EquipSlot]);

/// Slot reference with lifetime to enforce no modification while this is held.
#[derive(Clone, Copy)]
pub enum FoundSlot<'a> {
    Equipped(usize),
    Container(usize, usize, PhantomData<&'a ()>),
}

pub struct FoundSlotMut<'a>(&'a mut Inventory2Component, FoundSlot<'a>);

impl Inventory2Component {
    pub fn new(equip_slots: usize) -> Self {
        Inventory2Component {
            equip_slots: repeat_with(|| EquipSlot::Empty).take(equip_slots).collect(),
            containers: Vec::new(),
        }
    }

    pub fn give_container(&mut self, container: Container) {
        self.containers.push(container);
    }

    /// Get `extra_hands` consecutive slots
    pub fn get_hauling_slots(&mut self, extra_hands: u16) -> Option<EquipSlots> {
        self.find_hauling_slot_range(extra_hands).map(move |range| {
            // safety: range returned by private fn
            let slots = unsafe { self.equip_slots.get_unchecked_mut(range) };

            EquipSlots(slots)
        })
    }

    pub fn has_hauling_slots(&self, extra_hands: u16) -> bool {
        self.find_hauling_slot_range(extra_hands).is_some()
    }

    fn find_hauling_slot_range(&self, extra_hands: u16) -> Option<Range<usize>> {
        let full_hand_count = (extra_hands + 1) as usize;

        let mut start_idx = 0;
        while let Some(pos) = self
            .equip_slots
            .iter()
            .skip(start_idx)
            .position(EquipSlot::is_empty)
        {
            start_idx += pos;

            // found a free slot, now check if consecutive empties follow it
            let consecutive = self
                .equip_slots
                .iter()
                .skip(start_idx)
                .take(full_hand_count)
                .take_while(|e| e.is_empty())
                .count();

            if consecutive == full_hand_count {
                // nice
                let range = start_idx..start_idx + full_hand_count;
                debug_assert_eq!(self.equip_slots[range.clone()].len(), full_hand_count);
                debug_assert!(self.equip_slots[range.clone()]
                    .iter()
                    .all(EquipSlot::is_empty));
                return Some(range);
            } else {
                // keep going
                start_idx += consecutive;
            }
        }

        None
    }

    /// Removes the given item from any equip slots, returning the count. 0 if it wasn't there
    pub fn remove_item(&mut self, item: Entity) -> usize {
        self.equip_slots
            .iter_mut()
            .filter_map(|e| match *e {
                EquipSlot::Occupied(HeldEntity { entity, .. }) | EquipSlot::Overflow(entity)
                    if item == entity =>
                {
                    *e = EquipSlot::Empty;
                    Some(())
                }
                _ => None,
            })
            .count()
    }

    /// Clobbers the given range with Empty, asserting they aren't already empty
    fn empty_range(&mut self, range: Range<usize>) {
        self.equip_slots
            .get_mut(range)
            .unwrap()
            .iter_mut()
            .for_each(|e| {
                debug_assert!(!e.is_empty());
                *e = EquipSlot::Empty;
            })
    }

    pub fn equip_slots(&self) -> impl ExactSizeIterator<Item = &EquipSlot> + '_ {
        self.equip_slots.iter()
    }

    pub fn containers(&self) -> impl ExactSizeIterator<Item = &Container> + '_ {
        self.containers.iter()
    }

    pub fn containers_mut(&mut self) -> impl Iterator<Item = &mut Container> + '_ {
        self.containers.iter_mut()
    }

    fn get_first_held_range(&self) -> Option<(HeldEntity, Range<usize>)> {
        let (first_idx, _) = self
            .equip_slots
            .iter()
            .find_position(|e| matches!(e, EquipSlot::Occupied(_)))?;
        let overflow = self
            .equip_slots
            .iter()
            .skip(first_idx + 1)
            .take_while(|e| matches!(e, EquipSlot::Overflow(_)))
            .count();

        let range = first_idx..first_idx + overflow + 1;
        if cfg!(debug_assertions) {
            let slots = &self.equip_slots[range.clone()];
            let entity = match slots.get(0) {
                Some(EquipSlot::Occupied(HeldEntity { entity, .. })) => entity,
                _ => unreachable!(),
            };
            assert!(slots
                .iter()
                .skip(1)
                .all(|e| matches!(e, EquipSlot::Overflow(oe) if oe == entity)));
        }

        // safety: retrieved by find_position() above
        let held_entity = unsafe {
            match self.equip_slots.get_unchecked(range.start) {
                EquipSlot::Occupied(e) => e,
                _ => unreachable_unchecked(),
            }
        };

        Some((held_entity.to_owned(), range))
    }

    /// Frees up enough equip slots by pushing them into containers, then fills equip slots with
    /// the given item, returning true if it worked
    ///
    /// TODO it's possible some hands have been freed up while returning false anyway
    pub fn insert_item(
        &mut self,
        item: Entity,
        extra_hands: u16,
        volume: Volume,
        size: Length3,
    ) -> bool {
        // fast check we have enough slots
        if !self.fits_equip_slots(extra_hands) {
            return false;
        }

        loop {
            if let Some(mut slots) = self.get_hauling_slots(extra_hands) {
                // hands are free, pick up now
                slots.fill(item, volume, size);
                return true;
            }

            // need to free up slots
            let (item_to_move, item_range) = self
                .get_first_held_range()
                .expect("should have items remaining");

            // find container that can fit this item and move it
            if self
                .containers
                .iter_mut()
                .any(|container| container.add(&item_to_move).is_ok())
            {
                // excellent, we found one and moved the item into it, remove it from the equipped range
                self.empty_range(item_range);
                continue; // try again
            } else {
                // no containers found for this item
                // TODO loop along all held items rather than only checking the first
                // TODO configurable drop equipped items to make space instead of failing
                return false;
            }
        }
    }

    /// Checks there are enough equip slots regardless of current state
    fn fits_equip_slots(&self, extra_hands: u16) -> bool {
        self.equip_slots.len() >= (1 + extra_hands as usize)
    }

    pub fn search<W: ComponentWorld>(&self, filter: &ItemFilter, world: &W) -> Option<FoundSlot> {
        // TODO possibly add search cache keyed by entity, if there are many repeated searches for the same entity

        self.equip_slots
            .iter()
            .enumerate()
            .find_map(|(i, e)| {
                let entity = e.ok();
                (entity, Some(world))
                    .matches(*filter)
                    .as_some(FoundSlot::Equipped(i))
            })
            .or_else(|| {
                self.containers
                    .iter()
                    .enumerate()
                    .find_map(|(i, container)| {
                        container.contents().enumerate().find_map(|(j, entity)| {
                            (Some(entity.entity), Some(world))
                                .matches(*filter)
                                .as_some(FoundSlot::Container(i, j, PhantomData))
                        })
                    })
            })
    }
    pub fn search_mut<W: ComponentWorld>(
        &mut self,
        filter: &ItemFilter,
        world: &W,
    ) -> Option<FoundSlotMut> {
        let slot = self.search(filter, world)?;
        // safety: move slot lifetime alongside inventory lifetime in FoundSlotMut
        let slot = unsafe { std::mem::transmute(slot) };
        Some(FoundSlotMut(self, slot))
    }

    fn all_items(&self) -> impl Iterator<Item = Entity> + '_ {
        let equipped = self.equip_slots.iter().filter_map(|e| e.ok());
        let containers = self
            .containers()
            .flat_map(|container| container.contents().map(|e| e.entity));
        equipped.chain(containers)
    }

    /// Asserts all items dont have transforms, aren't duplicates, are alive, and that containers
    /// capacities are accurate
    /// - held_entities: item->holder
    #[cfg(debug_assertions)]
    pub fn validate(
        &self,
        holder: Entity,
        world: &impl ComponentWorld,
        held_entities: &mut HashMap<Entity, Entity>,
    ) {
        for e in self.all_items() {
            assert!(world.is_entity_alive(e), "item {} is dead", E(e));

            if let Some(other_holder) = held_entities.insert(e, holder) {
                panic!(
                    "item {} is in the inventories of {} and {}",
                    E(e),
                    E(holder),
                    E(other_holder)
                );
            }

            assert!(
                world.component::<TransformComponent>(e).is_err(),
                "held item {} has a transform",
                E(e)
            );
        }

        for container in &self.containers {
            let real_capacity: Volume = container
                .contents()
                .fold(Volume::new(0), |acc, e| acc + e.volume);

            assert_eq!(
                real_capacity,
                container.current_capacity(),
                "container reports a capacity of {} when it actually is {}",
                container.current_capacity(),
                real_capacity
            );
        }
    }
}

impl<'a> FoundSlotMut<'a> {
    /// Remove the item represented by this FoundSlot from its container, make space in equip slots
    /// then put in equip slots. If that fails it's put back into the container and returns false
    pub fn equip(self, extra_hands: u16) -> bool {
        let FoundSlotMut(inv, slot) = self;

        match slot {
            FoundSlot::Equipped(_) => {
                // already equipped
                true
            }
            FoundSlot::Container(container_idx, slot, _) => {
                // remove from container while this slot reference is valid
                // safety: indices from FoundSlot and inventory hasn't been mutated
                let removed_item = unsafe {
                    debug_assert!(inv.containers.get(container_idx).is_some());
                    let container = inv.containers.get_unchecked_mut(container_idx);
                    container.remove_at_index(slot)
                };

                // now attempt to free up equip slot
                if inv.insert_item(
                    removed_item.entity,
                    extra_hands,
                    removed_item.volume,
                    removed_item.half_dims,
                ) {
                    // it worked
                    true
                } else {
                    // failed to free up enough space, put item back into original container
                    // safety: no containers have been added/removed since
                    let container = unsafe { inv.containers.get_unchecked_mut(container_idx) };

                    let added = container.add(&removed_item);
                    debug_assert!(added.is_ok());
                    false
                }
            }
        }
    }
}

impl<'a> FoundSlot<'a> {
    /// Panics if entity not found, if for example this is not the same inventory the slot was
    /// resolved in
    pub fn get(self, inventory: &'a Inventory2Component) -> Entity {
        let entity = match self {
            FoundSlot::Equipped(i) => inventory.equip_slots.get(i).and_then(|e| e.ok()),
            FoundSlot::Container(i, j, _) => inventory
                .containers
                .get(i)
                .and_then(|c| c.contents_as_slice().get(j))
                .map(|e| e.entity),
        };

        entity.expect("entity should exist in inventory")
    }
}

impl<'a> EquipSlots<'a> {
    pub fn fill(&mut self, entity: Entity, volume: Volume, half_dims: Length3) {
        let mut slots = self.0.iter_mut();

        if let Some(e) = slots.next() {
            debug_assert!(e.is_empty());
            *e = EquipSlot::Occupied(HeldEntity {
                entity,
                volume,
                half_dims,
            });
        }
        slots.for_each(|e| {
            debug_assert!(e.is_empty());
            *e = EquipSlot::Overflow(entity)
        });
    }
}

impl Debug for FoundSlot<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FoundSlot::Equipped(i) => write!(f, "Equipped({})", i),
            FoundSlot::Container(container, slot, _) => {
                write!(f, "Container({}:{})", container, slot)
            }
        }
    }
}

impl<V: Value> ComponentTemplate<V> for InventoryComponentTemplate {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let equip_slots = values.get_int("equip_slots")?;
        Ok(Box::new(Self { equip_slots }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(Inventory2Component::new(self.equip_slots))
    }
}

register_component_template!("inventory", InventoryComponentTemplate);
