use std::collections::HashMap;
use std::convert::TryFrom;

use common::*;

use crate::ecs::world::{ComponentRefErased, SpecsWorld};
use crate::{EcsWorld, Entity};

#[derive(Debug, Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ComponentBuildError {
    #[error("Component is not buildable")]
    NotImplemented,

    #[error("Failed to deserialize ron: {0}")]
    Deserialize(#[from] ron::Error),

    #[error("Key {0:?} not found")]
    KeyNotFound(String),

    #[error("Failed to convert i64 {1} into type {0:?}")]
    InvalidIntValue(String, i64),

    #[error("Failed to convert f64 {1} into type {0:?}")]
    InvalidFloatValue(String, f64),

    #[error("Bad enum variant {0:?} for enum {1:?}")]
    InvalidEnumVariant(String, &'static str),

    // TODO should be a Box<dyn Error>
    #[error("Template error: {0}")]
    TemplateSpecific(String),

    #[error("Expected lowercase string but got {0:?}")]
    NotLowercase(String),

    #[error("Percentage should be 0-100 but is {0}")]
    BadPercentage(i64),

    #[error("Unexpected tag value {0:?}")]
    UnexpectedTagValue(String),
}

#[derive(Debug)]
pub struct Map<V: Value> {
    map: HashMap<String, V>,
}

pub trait Value: Debug {
    fn into_int(self) -> Result<i64, ComponentBuildError>;
    fn into_float(self) -> Result<f64, ComponentBuildError>;
    fn into_string(self) -> Result<String, ComponentBuildError>;
    fn into_unit(self) -> Result<(), ComponentBuildError>;

    fn as_unit(&self) -> Result<(), ComponentBuildError>;
    fn as_int(&self) -> Result<i64, ComponentBuildError>;
    fn into_type<T: serde::de::DeserializeOwned>(self) -> Result<T, ComponentBuildError>;
}

/// Reflection-like functionality through the ui, optionally implemented by components
/// TODO implement InteractiveComponent for some components
pub trait InteractiveComponent {
    fn as_debug(&self) -> Option<&dyn Debug>;
}

pub type HasCompFn = fn(&EcsWorld, Entity) -> bool;
pub type RegisterCompFn = fn(&mut SpecsWorld);
pub type GetComponentFn = fn(&EcsWorld, Entity) -> Option<ComponentRefErased>;
pub type AsInteractiveFn = unsafe fn(&()) -> Option<&dyn InteractiveComponent>;

pub struct ComponentEntry {
    pub name: &'static str,
    pub has_comp_fn: HasCompFn,
    pub register_comp_fn: RegisterCompFn,
    pub get_comp_fn: GetComponentFn,
}

inventory::collect!(ComponentEntry);

pub struct ComponentFunctions {
    has_comp: HasCompFn,
    get_comp: GetComponentFn,
}

pub struct ComponentRegistry {
    // TODO perfect hashing
    map: HashMap<&'static str, ComponentFunctions>,
}

impl<V: Value> Map<V> {
    pub fn empty() -> Self {
        Self {
            map: HashMap::with_capacity(0),
        }
    }

    pub fn from_fields<I: Into<V>>(fields: impl Iterator<Item = (String, I)>) -> Self {
        Self {
            map: fields.map(|(name, val)| (name, val.into())).collect(),
        }
    }

    pub fn get(&mut self, key: &str) -> Result<V, ComponentBuildError> {
        self.map
            .remove(key)
            .ok_or_else(|| ComponentBuildError::KeyNotFound(key.to_owned()))
    }

    pub fn get_int<I: TryFrom<i64>>(&mut self, key: &str) -> Result<I, ComponentBuildError> {
        self.get(key).and_then(|val| {
            val.into_int().and_then(|int| {
                I::try_from(int).map_err(|_| {
                    ComponentBuildError::InvalidIntValue(std::any::type_name::<I>().to_owned(), int)
                })
            })
        })
    }
    pub fn get_float<F: num_traits::NumCast>(
        &mut self,
        key: &str,
    ) -> Result<F, ComponentBuildError> {
        self.get(key).and_then(|val| {
            val.into_float().and_then(|float| {
                F::from(float).ok_or_else(|| {
                    ComponentBuildError::InvalidFloatValue(
                        std::any::type_name::<F>().to_owned(),
                        float,
                    )
                })
            })
        })
    }
    pub fn get_string(&mut self, key: &str) -> Result<String, ComponentBuildError> {
        self.get(key).and_then(|val| val.into_string())
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> + '_ {
        self.map.iter()
    }

    /// Only use for free-form structures where all keys are valid
    pub fn take(&mut self) -> Self {
        Self {
            map: std::mem::take(&mut self.map),
        }
    }
}

impl<V: Value> IntoIterator for Map<V> {
    type Item = (String, V);
    type IntoIter = std::collections::hash_map::IntoIter<String, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

impl ComponentRegistry {
    pub fn new(world: &mut SpecsWorld) -> Self {
        let mut map = HashMap::with_capacity(128);
        for comp in inventory::iter::<ComponentEntry> {
            debug!("registering component {:?}", comp.name);
            let old = map.insert(
                comp.name,
                ComponentFunctions {
                    has_comp: comp.has_comp_fn,
                    get_comp: comp.get_comp_fn,
                },
            );

            if old.is_some() {
                panic!("duplicate component with name {:?}", comp.name)
            }

            (comp.register_comp_fn)(world);
        }

        info!("registered {} components", map.len());
        map.shrink_to_fit();
        ComponentRegistry { map }
    }

    pub fn has_component(&self, comp: &str, world: &EcsWorld, entity: Entity) -> bool {
        match self.map.get(comp) {
            Some(funcs) => (funcs.has_comp)(world, entity),
            None => {
                warn!("looking up non-existent component {:?}", comp);
                if cfg!(debug_assertions) {
                    panic!("looking up non-existent component {:?}", comp)
                }
                false
            }
        }
    }

    /// Iterates through all known component types and checks each one
    pub fn all_components_for<'a>(
        &'a self,
        world: &'a EcsWorld,
        entity: Entity,
    ) -> impl Iterator<Item = (&'static str, ComponentRefErased)> + 'a {
        self.map.iter().filter_map(move |(name, funcs)| {
            (funcs.get_comp)(world, entity).map(|comp| (*name, comp))
        })
    }
}
