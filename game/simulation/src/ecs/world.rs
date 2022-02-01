use std::any::TypeId;
use std::hint::unreachable_unchecked;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use specs::prelude::Resource;
use specs::storage::InsertResult;
use specs::world::EntitiesRes;
use specs::LazyUpdate;

use common::*;

use crate::build::BuildTemplate;
use crate::definitions::{DefinitionBuilder, DefinitionErrorKind, DefinitionRegistry};
use crate::ecs::component::{AsInteractiveFn, ComponentRegistry};
use crate::ecs::*;
use crate::event::{DeathReason, EntityEvent, EntityEventQueue};
use crate::item::{ContainerComponent, ContainerResolver};
use crate::spatial::Spatial;
use crate::string::CachedStr;
use crate::{Entity, InnerWorldRef, ItemStackComponent, TransformComponent, WorldRef};

pub type SpecsWorld = specs::World;
pub struct EcsWorld {
    world: SpecsWorld,
    component_registry: ComponentRegistry,
    /// (definition name, build template, rendered KindComponent)
    build_templates: Vec<(CachedStr, Rc<BuildTemplate>, Option<String>)>,
}

pub struct CachedWorldRef<'a> {
    ecs: &'a EcsWorld,
    voxel_world: Option<(WorldRef, InnerWorldRef<'a>)>,
}

#[derive(Debug, Error)]
pub enum ComponentGetError {
    #[error("The entity {} doesn't exist", *.0)]
    NoSuchEntity(Entity),

    #[error("The entity {} doesn't have the given component '{1}'", *.0)]
    NoSuchComponent(Entity, &'static str),
}

#[derive(Debug, Error)]
#[error("There are build templates with invalid materials: {0:?}")]
pub struct InvalidBuildTemplatesError(Vec<String>);

/// Resource to hold entities to kill
#[derive(Default)]
pub struct EntitiesToKill {
    entities: Vec<specs::Entity>,
    reasons: Vec<DeathReason>,
}

pub trait ComponentWorld: ContainerResolver + Sized {
    fn component<T: Component>(
        &self,
        entity: Entity,
    ) -> Result<ComponentRef<'_, T>, ComponentGetError>;
    fn component_mut<T: Component>(
        &self,
        entity: Entity,
    ) -> Result<ComponentRefMut<'_, T>, ComponentGetError>;
    fn has_component<T: Component>(&self, entity: Entity) -> bool;
    fn has_component_by_name(&self, comp: &str, entity: Entity) -> bool;
    fn components<J: Join>(&self, entity: Entity, storages: J) -> Option<J::Type>;

    fn resource<T: Resource>(&self) -> &T;
    #[allow(clippy::mut_from_ref)]
    fn resource_mut<T: Resource>(&self) -> &mut T;

    fn add_now<T: Component>(&self, entity: Entity, component: T) -> InsertResult<T>;
    fn remove_now<T: Component>(&self, entity: Entity) -> Option<T>;

    fn add_lazy<T: Component>(&self, entity: Entity, component: T);

    fn remove_lazy<T: Component>(&self, entity: Entity);

    fn voxel_world(&self) -> WorldRef;

    fn build_entity(
        &self,
        definition_uid: &str,
    ) -> Result<DefinitionBuilder<Self>, DefinitionErrorKind>;

    /// From specs:
    /// > You have to make sure that no component storage is borrowed during the building!
    fn create_entity(&self) -> EntityBuilder;

    fn kill_entity(&self, entity: Entity, reason: DeathReason);
    fn is_entity_alive(&self, entity: Entity) -> bool;

    /// Called when an entity is spawned (before components are added)
    fn on_new_entity_creation(&self, entity: Entity);

    // ---
    fn kill_entities(&self, entities: &[Entity], reason: DeathReason) {
        for e in entities {
            self.kill_entity(*e, reason)
        }
    }

    fn mk_component_error<T: Component>(&self, entity: Entity) -> ComponentGetError {
        if self.is_entity_alive(entity) {
            ComponentGetError::no_such_component::<T>(entity)
        } else {
            ComponentGetError::NoSuchEntity(entity)
        }
    }

    fn post_event(&self, event: EntityEvent) {
        let queue = self.resource_mut::<EntityEventQueue>();
        queue.post(event)
    }
}

impl EntitiesToKill {
    // pub fn add(&mut self, entity: Entity, reason: DeathReason) {
    //     self.entities.push(entity.into());
    //     self.reasons.push(reason);
    // }

    pub fn add_many(&mut self, entities: impl Iterator<Item = (Entity, DeathReason)>) {
        if let Some(n) = entities.size_hint().1 {
            self.entities.reserve(n);
            self.reasons.reserve(n);
        }

        for (entity, reason) in entities {
            self.entities.push(entity.into());
            self.reasons.push(reason);
        }
    }

    pub fn replace_entities(&mut self, replacement: Vec<specs::Entity>) -> Vec<specs::Entity> {
        std::mem::replace(&mut self.entities, replacement)
    }

    pub fn count(&self) -> usize {
        debug_assert_eq!(self.entities.len(), self.reasons.len());
        self.entities.len()
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        self.reasons.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, DeathReason)> + '_ {
        self.entities
            .iter()
            .copied()
            .map(Entity::from)
            .zip(self.reasons.iter().copied())
    }
}

/// Component reference that keeps a ReadStorage instance
#[derive(Clone)]
#[repr(C)] // component should be first member
pub struct ComponentRef<'a, T: Component> {
    comp: &'a T,
    storage: ReadStorage<'a, T>,
}

#[derive(Component)]
struct DummyComponent;

/// Assumed (and checked) to be the same size for all types
const SPECS_READSTORAGE_SIZE: usize = std::mem::size_of::<ReadStorage<DummyComponent>>();

/// Type-erased component ref
pub struct ComponentRefErased<'a> {
    /// Original ReadStorage<'a, T>
    storage: [u8; SPECS_READSTORAGE_SIZE],
    /// std::ptr::drop_in_place for storage
    storage_drop: unsafe fn(*mut u8),
    comp: *const (),
    comp_typeid: TypeId,
    as_interactive: AsInteractiveFn,
    dummy: PhantomData<&'a ()>,
}

/// A reference to an inner field of a component held in a [ComponentRef]
#[derive(Clone)]
#[repr(C)] // value should be first member
pub struct ComponentRefMapped<'a, T: Component, U> {
    value: &'a U,
    storage: ReadStorage<'a, T>,
}

/// Component reference that keeps a ReadStorage instance
#[repr(C)] // component should be first member
pub struct ComponentRefMut<'a, T: Component> {
    comp: &'a mut T,
    storage: WriteStorage<'a, T>,
}

/// A reference to an inner field of a component held in a [ComponentRef]
#[repr(C)] // value should be first member
pub struct ComponentRefMutMapped<'a, T: Component, U> {
    value: &'a mut U,
    storage: WriteStorage<'a, T>,
}

impl<W: ComponentWorld> ContainerResolver for W {
    fn container(&self, e: Entity) -> Option<ComponentRef<'_, ContainerComponent>> {
        self.component(e).ok()
    }

    fn container_mut(&self, e: Entity) -> Option<ComponentRefMut<'_, ContainerComponent>> {
        self.component_mut(e).ok()
    }
}

impl Deref for EcsWorld {
    type Target = SpecsWorld;

    fn deref(&self) -> &Self::Target {
        &self.world
    }
}

impl DerefMut for EcsWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.world
    }
}

impl EcsWorld {
    pub fn with_definitions(
        definitions: DefinitionRegistry,
    ) -> Result<Self, InvalidBuildTemplatesError> {
        let mut world = SpecsWorld::new();
        let reg = ComponentRegistry::new(&mut world);

        // collect and validate build templates
        let build_templates = {
            let templates = definitions
                .iter_templates::<BuildTemplate>("build")
                .filter_map(|def| {
                    let definition = definitions.lookup_definition(def)?;

                    let build = definition.find_component_ref::<BuildTemplate>("build")?;

                    let name = definition
                        .find_component("kind")
                        .and_then(|any| any.downcast_ref::<KindComponent>())
                        .map(|kind| format!("{}", kind));

                    Some((def, build, name))
                })
                .collect_vec();

            let mut invalids = Vec::new();
            for (def, build, _) in &templates {
                for mat in build.materials() {
                    if definitions.lookup_definition(mat.definition()).is_none() {
                        invalids.push(format!("{}:{}", def, mat.definition()));
                    }
                }
            }

            if invalids.is_empty() {
                templates
            } else {
                return Err(InvalidBuildTemplatesError(invalids));
            }
        };

        let mut world = EcsWorld {
            world,
            component_registry: reg,
            build_templates,
        };

        world.world.insert(definitions);
        Ok(world)
    }

    #[cfg(test)]
    pub fn new() -> Self {
        let reg = crate::definitions::load_from_str("[]").expect("can't load null definitions");
        Self::with_definitions(reg).expect("invalid definitions")
    }

    /// Iterates through all known component types and checks each one
    pub fn all_components_for(
        &self,
        entity: Entity,
    ) -> impl Iterator<Item = (&'static str, ComponentRefErased)> {
        self.component_registry.all_components_for(self, entity)
    }

    /// Returns Err if either entity is not alive.
    /// Only components not marked as `#[clone(disallow)]`
    pub fn copy_components_to(&self, source: Entity, dest: Entity) -> Result<(), Entity> {
        self.component_registry
            .copy_components_to(self, source, dest)
    }

    /// Returns the name of the first non-copyable component that this entity has
    pub fn find_non_copyable(&self, entity: Entity) -> Option<&'static str> {
        self.component_registry.find_non_copyable(self, entity)
    }

    /// (definition name, template, build name)
    pub fn build_templates(&self) -> &[(CachedStr, Rc<BuildTemplate>, Option<String>)] {
        &self.build_templates
    }

    pub fn find_build_template(&self, name: &str) -> Option<Rc<BuildTemplate>> {
        self.build_templates.iter().find_map(|(def, template, _)| {
            if def.as_ref() == name {
                Some(template.clone())
            } else {
                None
            }
        })
    }
}

impl ComponentWorld for EcsWorld {
    fn component<T: Component>(
        &self,
        entity: Entity,
    ) -> Result<ComponentRef<'_, T>, ComponentGetError> {
        let storage = self.read_storage::<T>();
        let comp = storage
            .get(entity.into())
            .ok_or_else(|| self.mk_component_error::<T>(entity))?;

        let comp = unsafe { std::mem::transmute::<&T, &T>(comp) };
        Ok(ComponentRef { storage, comp })
    }

    fn component_mut<T: Component>(
        &self,
        entity: Entity,
    ) -> Result<ComponentRefMut<'_, T>, ComponentGetError> {
        let mut storage = self.write_storage::<T>();
        let comp = storage
            .get_mut(entity.into())
            .ok_or_else(|| self.mk_component_error::<T>(entity))?;

        let comp = unsafe { std::mem::transmute::<&mut T, &mut T>(comp) };
        Ok(ComponentRefMut { storage, comp })
    }

    fn has_component<T: Component>(&self, entity: Entity) -> bool {
        let storage = self.read_storage::<T>();
        storage.contains(entity.into())
    }

    fn has_component_by_name(&self, comp: &str, entity: Entity) -> bool {
        self.component_registry.has_component(comp, self, entity)
    }

    fn components<J: Join>(&self, entity: Entity, storages: J) -> Option<J::Type> {
        let entities = self.read_resource::<EntitiesRes>();
        storages.join().get(entity.into(), &entities.into())
    }

    fn resource<T: Resource>(&self) -> &T {
        let res = self.read_resource::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the resource's
        // lifetime from that of the storage to that of self
        unsafe { std::mem::transmute(res.deref()) }
    }

    fn resource_mut<T: Resource>(&self) -> &mut T {
        let mut res = self.write_resource::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the components
        // lifetime from that of the storage to that of self
        let res: &mut T = unsafe { std::mem::transmute(res.deref_mut()) };
        res
    }

    fn add_now<T: Component>(&self, entity: Entity, component: T) -> InsertResult<T> {
        let mut storage = self.write_storage::<T>();
        storage.insert(entity.into(), component)
    }

    fn remove_now<T: Component>(&self, entity: Entity) -> Option<T> {
        let mut storage = self.write_storage::<T>();
        storage.remove(entity.into())
    }

    // TODO specs lazy updates allocs a Box for each action - when our QueuedUpdates uses an arena swap this out to use that instead
    fn add_lazy<T: Component>(&self, entity: Entity, component: T) {
        let lazy = self.read_resource::<LazyUpdate>();
        lazy.insert(entity.into(), component);
    }

    fn remove_lazy<T: Component>(&self, entity: Entity) {
        let lazy = self.read_resource::<LazyUpdate>();
        lazy.remove::<T>(entity.into());
    }

    fn voxel_world(&self) -> WorldRef {
        (*self.read_resource::<WorldRef>()).clone()
    }

    fn build_entity(
        &self,
        definition_uid: &str,
    ) -> Result<DefinitionBuilder<Self>, DefinitionErrorKind> {
        let definitions = self.resource::<DefinitionRegistry>();
        definitions.instantiate(&*definition_uid, self)
    }

    fn create_entity(&self) -> EntityBuilder {
        let builder = WorldExt::create_entity_unchecked(&self.world);
        self.on_new_entity_creation(builder.entity.into());
        builder
    }

    fn kill_entity(&self, entity: Entity, reason: DeathReason) {
        let mut to_kill = SmallVec::<[(Entity, DeathReason); 1]>::new();
        to_kill.push((entity, reason));
        trace!("killing entity"; entity);

        // special case for item stacks, destroy all contained items too
        if let Ok(stack) = self.component::<ItemStackComponent>(entity) {
            to_kill.extend(
                stack
                    .stack
                    .contents()
                    .map(|(e, _)| (e, DeathReason::ParentStackDestroyed)),
            );
            trace!("killing item stack contents too"; "stack" => entity, "contents" => ?stack.stack.contents().collect_vec());
        }

        // scatter items from a container around it
        if let Ok(container) = self.component::<ContainerComponent>(entity) {
            // remove all items from container
            let container_pos = self
                .component::<TransformComponent>(entity)
                .map(|t| t.position);
            match container_pos {
                Ok(scatter_around) => {
                    let mut rng = thread_rng();
                    for item in container.container.contents().map(|e| e.entity) {
                        self.helpers_comps().remove_from_container(item);

                        // scatter items around
                        // TODO move item scattering to a utility function
                        let scatter_pos = {
                            let offset_x = rng.gen_range(-0.3, 0.3);
                            let offset_y = rng.gen_range(-0.3, 0.3);
                            scatter_around + (offset_x, offset_y, 0.0)
                        };

                        let _ = self.add_now(item, TransformComponent::new(scatter_pos));
                    }
                }
                Err(err) => {
                    error!("container destroyed with no transform, cannot drop its items"; "err" => %err);
                }
            }
        }

        // kill before next maintain
        let deathlist = self.resource_mut::<EntitiesToKill>();
        deathlist.add_many(to_kill.into_iter());
    }

    fn is_entity_alive(&self, entity: Entity) -> bool {
        // must check if generation is alive first to avoid panic
        entity.gen().is_alive() && self.is_alive(entity.into())
    }

    fn on_new_entity_creation(&self, entity: Entity) {
        // include this entity in spatial queries even before the system updates
        if let Some(spatial) = self.try_fetch::<Spatial>() {
            spatial.register_new_entity(entity);
        }
    }
}

impl ComponentGetError {
    fn no_such_component<T>(entity: Entity) -> Self {
        Self::NoSuchComponent(entity, std::any::type_name::<T>())
    }
}

impl<T: Component> Deref for ComponentRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.comp
    }
}

impl<T: Component> Deref for ComponentRefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.comp
    }
}

impl<T: Component> DerefMut for ComponentRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.comp
    }
}

impl<'a, T: Component> ComponentRef<'a, T> {
    pub fn map<U>(&self, f: impl FnOnce(&'a T) -> &'a U) -> ComponentRefMapped<'a, T, U> {
        let new_val = f(self.comp);
        ComponentRefMapped {
            value: new_val,
            storage: self.storage.clone(),
        }
    }

    pub fn erased(self, as_interactive: AsInteractiveFn) -> ComponentRefErased<'a> {
        let storage_drop = {
            let drop = std::ptr::drop_in_place::<ReadStorage<'a, T>>;
            // erase type from destructor fn ptr
            unsafe {
                std::mem::transmute::<unsafe fn(*mut ReadStorage<'a, T>), unsafe fn(*mut u8)>(drop)
            }
        };

        // copy storage struct into a vec of bytes to erase the type
        let storage = {
            let original_storage = ManuallyDrop::new(self.storage);
            let readstorage_size = std::mem::size_of::<ReadStorage<'a, T>>();
            assert_eq!(readstorage_size, SPECS_READSTORAGE_SIZE);

            let mut storage = [0u8; SPECS_READSTORAGE_SIZE];
            let src_ptr = &*original_storage as *const ReadStorage<'a, T> as *const u8;
            let src_slice = unsafe {
                std::slice::from_raw_parts(src_ptr, std::mem::size_of::<ReadStorage<'a, T>>())
            };

            storage.copy_from_slice(src_slice);
            storage
        };

        let comp = self.comp as *const _ as *const ();
        let comp_typeid = TypeId::of::<T>();

        ComponentRefErased {
            storage,
            storage_drop,
            comp,
            comp_typeid,
            as_interactive,
            dummy: PhantomData,
        }
    }
}

impl<'a, T: Component + Debug> Debug for ComponentRef<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self.comp, f)
    }
}

impl<'a, T: Component + Display> Display for ComponentRef<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self.comp, f)
    }
}

impl<T: Component, U> Deref for ComponentRefMapped<'_, T, U> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T: Component> ComponentRefMut<'a, T> {
    pub fn map<U>(self, f: impl FnOnce(&'a mut T) -> &'a mut U) -> ComponentRefMutMapped<'a, T, U> {
        let new_val = f(self.comp);
        ComponentRefMutMapped {
            value: new_val,
            storage: self.storage,
        }
    }
}

impl<T: Component, U> Deref for ComponentRefMutMapped<'_, T, U> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T: Component, U> DerefMut for ComponentRefMutMapped<'_, T, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<'a> ComponentRefErased<'a> {
    pub fn downcast<T: 'static>(&self) -> Option<&'_ T> {
        if TypeId::of::<T>() == self.comp_typeid {
            // safety: checked type id
            Some(unsafe { &*(self.comp as *const T) })
        } else {
            None
        }
    }

    pub fn as_interactive(&self) -> Option<&dyn InteractiveComponent> {
        unsafe { (self.as_interactive)(&*self.comp) }
    }
}

impl Drop for ComponentRefErased<'_> {
    fn drop(&mut self) {
        unsafe { (self.storage_drop)(self.storage.as_mut_ptr()) }
    }
}

impl<'a> CachedWorldRef<'a> {
    pub fn new(ecs: &'a EcsWorld) -> Self {
        Self {
            ecs,
            voxel_world: None,
        }
    }

    pub fn get(&mut self) -> &'_ InnerWorldRef<'a> {
        if self.voxel_world.is_none() {
            // init world ref and store
            let world = self.ecs.voxel_world();
            let world_ref = world.borrow();

            // safety: ref lives as long as self
            let world_ref =
                unsafe { std::mem::transmute::<InnerWorldRef, InnerWorldRef>(world_ref) };
            self.voxel_world = Some((world, world_ref));
        }

        match self.voxel_world.as_ref() {
            Some((_, w)) => w,
            _ => {
                // safety: unconditionally initialised
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ecs::*;

    use super::*;

    #[derive(Debug, Component, EcsComponent, Clone)]
    #[storage(VecStorage)]
    #[interactive]
    #[name("awesome")]
    pub struct TestInteractiveComponent(u64);

    impl InteractiveComponent for TestInteractiveComponent {
        fn as_debug(&self) -> Option<&dyn Debug> {
            Some(self as &dyn Debug)
        }
    }

    #[test]
    fn erased_component_ref() {
        let ecs = EcsWorld::new();
        let magic = 0x1213_1415_1617_1819;
        let entity = ecs
            .create_entity()
            .with(TestInteractiveComponent(magic))
            .build();

        let comp_ref = ecs
            .component::<TestInteractiveComponent>(entity.into())
            .unwrap();

        let erased = comp_ref.erased(TestInteractiveComponent::as_interactive); // autogenerated

        // ensure we can still get the original ref back
        let original = erased
            .downcast::<TestInteractiveComponent>()
            .expect("downcast failed");
        assert_eq!(original.0, magic);

        // ensure we cant cast to anything else
        assert!(erased.downcast::<String>().is_none());

        let interactive = erased.as_interactive();
        match interactive {
            Some(interactive) => {
                let debug = interactive.as_debug().unwrap();
                assert_eq!(
                    format!("{:?}", debug),
                    format!("TestInteractiveComponent({})", magic)
                );
            }
            _ => unreachable!(),
        }

        // drop ReadStorage
        drop(erased);

        // read storage ref shouldve been dropped, now we can get a mutable ref to it
        let _ = ecs.write_storage::<TestInteractiveComponent>();
    }

    /// Soundness confirmed by miri
    #[test]
    fn cached_world_ref() {
        let mut ecs = EcsWorld::new();
        ecs.insert(WorldRef::default());

        let mut lazy = CachedWorldRef::new(&ecs);

        let a = lazy.get();
        let b = lazy.get();
    }
}
