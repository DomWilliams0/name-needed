use std::hint::unreachable_unchecked;
use std::iter::repeat_with;
use std::ops::{Deref, DerefMut, Range};

use common::*;
use unit::space::length::Length3;
use unit::space::volume::Volume;

use crate::ecs::*;
use crate::SocietyHandle;

use crate::item::inventory::equip::EquipSlot;
use crate::item::inventory::{Container, HeldEntity};
use crate::item::{ItemFilter, ItemFilterable};

/// Temporary dumb component to hold equip slots and containers. Will eventually be a view on top of
/// the physical body tree
#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("inventory")]
#[clone(disallow)]
pub struct InventoryComponent {
    equip_slots: Vec<EquipSlot>,

    /// Each entity has a ContainerComponent
    containers: Vec<Entity>,
}

/// Inventory of a container
#[derive(Component, EcsComponent)]
#[storage(HashMapStorage)]
#[name("container")]
#[clone(disallow)]
pub struct ContainerComponent {
    pub container: Container,

    /// If set, appears in the owner's private stash
    // TODO owner should be handled in the same way as communal i.e. mirror state elsewhere
    pub owner: Option<Entity>,

    /// If set, appears in the society's stash. This field only mirrors the state in the society
    communal: Option<SocietyHandle>,
}

#[derive(Debug)]
pub struct InventoryComponentTemplate {
    equip_slots: usize,
}

#[derive(Debug)]
pub struct ContainerComponentTemplate {
    volume: Volume,
    size: Length3,
}

pub struct EquipSlots<'a>(&'a mut [EquipSlot]);

/// Slot reference with lifetime to enforce no modification while this is held.
#[derive(Clone, Copy)]
pub enum FoundSlot<'a> {
    /// Slot index
    Equipped(usize),
    /// (container entity, index)
    Container(Entity, usize, PhantomData<&'a ()>),
}

pub struct FoundSlotMut<'a>(&'a mut InventoryComponent, FoundSlot<'a>);

pub trait ContainerResolver {
    fn container(&self, e: Entity) -> Option<ComponentRef<'_, ContainerComponent>>;
    fn container_mut(&self, e: Entity) -> Option<ComponentRefMut<'_, ContainerComponent>>;

    fn container_unchecked(&self, e: Entity) -> ComponentRef<'_, ContainerComponent> {
        self.container(e).expect("entity does not have container")
    }

    #[allow(clippy::mut_from_ref)]
    fn container_mut_unchecked(&self, e: Entity) -> ComponentRefMut<'_, ContainerComponent> {
        self.container_mut(e)
            .expect("entity does not have container")
    }
}

impl InventoryComponent {
    pub fn new(equip_slots: usize) -> Self {
        InventoryComponent {
            equip_slots: repeat_with(|| EquipSlot::Empty).take(equip_slots).collect(),
            containers: Vec::new(),
        }
    }

    /// Entity must have a container component
    pub fn give_container(&mut self, container: Entity) {
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

    /// Finds range of empty slots to fit the number of extra hands
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

    /// Largest number of TOTAL hands available for hauling
    pub fn total_hands_available(&self) -> Option<u16> {
        let total_extra_hands = {
            let total_hands = self.equip_slots.len();
            if total_hands == 0 {
                // no extra hands
                return None;
            } else {
                (total_hands - 1) as u16
            }
        };

        // this could definitely be done better
        for extra_hands in (0..=total_extra_hands).rev() {
            if self.has_hauling_slots(extra_hands) {
                return Some(extra_hands + 1);
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

    pub fn has_equipped(&self, item: Entity) -> bool {
        self.search_equipped(ItemFilter::SpecificEntity(item), Option::<&EcsWorld>::None)
    }

    pub fn search_equipped(&self, filter: ItemFilter, world: Option<&impl ComponentWorld>) -> bool {
        self.equip_slots.iter().any(|slot| match slot {
            EquipSlot::Occupied(e) => (e.entity, world).matches(filter),
            _ => false,
        })
    }

    pub fn containers_unresolved(&self) -> impl ExactSizeIterator<Item = &Entity> + '_ {
        self.containers.iter()
    }

    /// Panics if any containers are missing container component
    pub fn containers<'a>(
        &'a self,
        resolver: &'a impl ContainerResolver,
    ) -> impl ExactSizeIterator<Item = (Entity, impl Deref<Target = Container> + 'a)> + '_ {
        self.containers
            .iter()
            .map(move |e| (*e, resolver.container_unchecked(*e).map(|c| &c.container)))
    }

    /// Panics if any containers are missing container component
    pub fn containers_mut<'a>(
        &'a mut self,
        resolver: &'a impl ContainerResolver,
    ) -> impl ExactSizeIterator<Item = (Entity, impl DerefMut<Target = Container> + 'a)> + '_ {
        self.containers.iter().map(move |e| {
            (
                *e,
                resolver
                    .container_mut_unchecked(*e)
                    .map(|c| &mut c.container),
            )
        })
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
    /// on_move: callback for moving item into a container (item, container)
    ///
    /// TODO it's possible some hands have been freed up while returning false anyway
    pub fn insert_item<R: ContainerResolver>(
        &mut self,
        resolver: &R,
        item: Entity,
        extra_hands: u16,
        volume: Volume,
        size: Length3,
        mut on_move: impl FnMut(Entity, Entity),
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
            let attempt_fit = |container: &&Entity| {
                resolver
                    .container_mut(**container)
                    .and_then(|mut comp| comp.container.add(&item_to_move).ok())
                    .is_some()
            };

            if let Some(container) = self.containers.iter().find(attempt_fit) {
                // excellent, we found one and moved the item into it, remove it from the equipped range
                on_move(item_to_move.entity, *container);
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

        let found_in_equipped = self.equip_slots.iter().enumerate().find_map(|(i, e)| {
            let entity = e.ok();
            (entity, Some(world))
                .matches(*filter)
                .as_some(FoundSlot::Equipped(i))
        });

        found_in_equipped.or_else(|| {
            self.containers(world).find_map(|(c, container)| {
                container.contents().enumerate().find_map(|(i, entity)| {
                    let filterable = (Some(entity.entity), Some(world));
                    let matches = ItemFilterable::matches(filterable, *filter);
                    matches.as_some(FoundSlot::Container(c, i, PhantomData))
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

    pub fn all_equipped_items(&self) -> impl Iterator<Item = Entity> + '_ {
        self.equip_slots.iter().filter_map(|e| e.ok())
    }
}

impl<'a> FoundSlotMut<'a> {
    /// Remove the item represented by this FoundSlot from its container, make space in equip slots
    /// then put in equip slots. If that fails it's put back into the container and returns false
    pub fn equip(self, extra_hands: u16, resolver: &impl ContainerResolver) -> bool {
        let FoundSlotMut(inv, slot) = self;

        match slot {
            FoundSlot::Equipped(_) => {
                // already equipped
                true
            }
            FoundSlot::Container(container, slot, _) => {
                let container_idx = inv
                    .containers
                    .iter()
                    .position(|e| *e == container)
                    .unwrap_or_else(|| {
                        panic!("FoundSlot container is incorrect (container {})", container)
                    });

                // remove from container while this slot reference is valid
                // safety: indices from FoundSlot and .position(), and inventory hasn't been mutated
                let removed_item = unsafe {
                    let container = inv.containers.get_unchecked(container_idx);
                    let container = &mut resolver.container_mut_unchecked(*container).container;
                    container.remove_at_index(slot)
                };

                // now attempt to free up equip slot
                if inv.insert_item(
                    resolver,
                    removed_item.entity,
                    extra_hands,
                    removed_item.volume,
                    removed_item.size,
                    |_item, _container| {
                        // TODO impl this when a scenario is found to hit this code path :^)
                        //  - pass through a closure from caller?
                        todo!()
                    },
                ) {
                    // it worked
                    true
                } else {
                    // failed to free up enough space, put item back into original container
                    // safety: no containers have been added/removed since
                    let container = unsafe {
                        let container = inv.containers.get_unchecked(container_idx);
                        &mut resolver.container_mut_unchecked(*container).container
                    };

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
    pub fn get(
        self,
        inventory: &'a InventoryComponent,
        resolver: &impl ContainerResolver,
    ) -> Entity {
        let entity = match self {
            FoundSlot::Equipped(i) => inventory.equip_slots.get(i).and_then(|e| e.ok()),
            FoundSlot::Container(c, i, _) => inventory
                .containers(resolver)
                .find(|(e, _)| *e == c)
                .and_then(|(_, c)| c.contents_as_slice().get(i).map(|e| e.entity)),
        };

        entity.expect("entity should exist in inventory")
    }
}

impl<'a> EquipSlots<'a> {
    pub fn fill(&mut self, entity: Entity, volume: Volume, size: Length3) {
        let mut slots = self.0.iter_mut();

        if let Some(e) = slots.next() {
            debug_assert!(e.is_empty());
            *e = EquipSlot::Occupied(HeldEntity {
                entity,
                volume,
                size,
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

#[cfg(debug_assertions)]
mod validation {
    use crate::item::inventory::HeldEntity;
    use crate::item::HauledItemComponent;
    use crate::{
        ComponentWorld, ContainedInComponent, Container, ContainerComponent, Entity,
        InventoryComponent, TransformComponent,
    };
    use std::collections::HashMap;
    use unit::space::volume::Volume;

    impl ContainerComponent {
        /// Asserts all items dont have transforms, aren't duplicates, are alive, and that containers
        /// are valid and their capacities accurate
        /// - held_entities: item->holder
        pub fn validate(
            &self,
            container: Entity,
            world: &impl ComponentWorld,
            held_entities: &mut HashMap<Entity, ContainedInComponent>,
        ) {
            validate_container(&self.container, container, held_entities, world);
        }
    }

    impl InventoryComponent {
        /// Asserts all items dont have transforms, aren't duplicates, are alive, and that containers
        /// are valid and their capacities accurate
        /// - held_entities: item->holder
        pub fn validate(
            &self,
            holder: Entity,
            world: &impl ComponentWorld,
            held_entities: &mut HashMap<Entity, ContainedInComponent>,
        ) {
            for e in self.all_equipped_items() {
                assert!(world.is_entity_alive(e), "item {} is dead", e);

                if let Some(other_holder) =
                    held_entities.insert(e, ContainedInComponent::InventoryOf(holder))
                {
                    panic!(
                        "item {} is in the inventory of {} and {}",
                        e, holder, other_holder,
                    );
                }

                let has_hauled = world.has_component::<HauledItemComponent>(e);
                let has_transform = world.has_component::<TransformComponent>(e);

                assert_eq!(
                    has_hauled, has_transform,
                    "equipped item {} is invalid (being hauled = {}, has transform = {})",
                    e, has_hauled, has_transform
                );
            }

            for (e, container) in self.containers(world) {
                if let Some(other_holder) =
                    held_entities.insert(e, ContainedInComponent::InventoryOf(holder))
                {
                    panic!(
                        "container {} is in the inventory of {} and {}",
                        e, holder, other_holder,
                    );
                }

                validate_container(&container, e, held_entities, world);
            }
        }
    }

    fn validate_container(
        container: &Container,
        container_entity: Entity,
        held_entities: &mut HashMap<Entity, ContainedInComponent>,
        world: &impl ComponentWorld,
    ) {
        for &HeldEntity { entity: e, .. } in container.contents() {
            assert!(world.is_entity_alive(e), "item {} is dead", e);

            if let Some(other_holder) =
                held_entities.insert(e, ContainedInComponent::Container(container_entity))
            {
                let contained = world.component::<ContainedInComponent>(e).ok();
                if let Some(contained) = contained {
                    // this container has already been visited in another inventory
                    let holder = contained.entity();
                    assert_eq!(
                        holder, container_entity,
                        "item {} found in container {} has invalid ContainedInComponent '{}'",
                        e, container_entity, *contained
                    );
                } else {
                    panic!(
                        "item {} is in the container {} and also {}",
                        e, container_entity, other_holder,
                    );
                }
            }

            assert!(
                !world.has_component::<TransformComponent>(e),
                "item {} in container has a transform",
                e
            );

            assert!(
                !world.has_component::<HauledItemComponent>(e),
                "item {} in container has a hauled component",
                e
            );

            let contained = world
                .component::<ContainedInComponent>(e)
                .unwrap_or_else(|_| {
                    panic!(
                        "item {} in container does not have a contained component",
                        e
                    )
                });

            let contained = contained.entity();
            assert_eq!(
                contained, container_entity,
                "item {} in container {} has a mismatching contained-in: {}",
                e, container_entity, contained,
            );
        }

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

impl ContainerComponent {
    pub fn communal(&self) -> Option<SocietyHandle> {
        self.communal
    }

    /// Must be kept in sync with society
    pub(in crate::item) fn make_communal(
        &mut self,
        society: Option<SocietyHandle>,
    ) -> Option<SocietyHandle> {
        std::mem::replace(&mut self.communal, society)
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
        builder.with(InventoryComponent::new(self.equip_slots))
    }

    crate::as_any!();
}

// TODO this is the same as is used by PhysicalComponent
#[derive(serde::Deserialize)]
struct SizeLimit {
    x: u16,
    y: u16,
    z: u16,
}

impl<V: Value> ComponentTemplate<V> for ContainerComponentTemplate {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let volume: u16 = values.get_int("volume")?;
        let size: SizeLimit = values.get("size")?.into_type()?;

        Ok(Box::new(ContainerComponentTemplate {
            volume: Volume::new(volume),
            size: Length3::new(size.x, size.y, size.z),
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(ContainerComponent {
            container: Container::new(self.volume, self.size),
            owner: None,
            communal: None,
        })
    }

    crate::as_any!();
}

register_component_template!("inventory", InventoryComponentTemplate);
register_component_template!("container", ContainerComponentTemplate);
