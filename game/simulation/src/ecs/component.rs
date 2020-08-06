use common::derive_more::{Display, Error, IntoIterator};
use common::*;

use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Debug, Display, Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ComponentBuildError {
    #[display(fmt = "Component is not buildable")]
    NotImplemented,

    #[display(fmt = "Failed to deserialize ron: {}", _0)]
    Deserialize(ron::Error),

    #[display(fmt = "Key {:?} not found", _0)]
    KeyNotFound(#[error(not(source))] String),

    #[display(fmt = "Failed to convert i64 {} into type {:?}", _1, _0)]
    InvalidIntValue(String, i64),

    #[display(fmt = "Failed to convert f64 {} into type {:?}", _1, _0)]
    InvalidFloatValue(String, f64),

    #[display(fmt = "Bad enum variant {:?} for enum {:?}", _0, _1)]
    InvalidEnumVariant(String, &'static str),

    // TODO should be a Box<dyn Error>
    #[display(fmt = "Template error: {}", _0)]
    TemplateSpecific(#[error(not(source))] String),
}

#[derive(Debug, IntoIterator)]
pub struct Map<V: Value> {
    map: HashMap<String, V>,
}

pub trait Value: Debug {
    fn into_int(self) -> Result<i64, ComponentBuildError>;
    fn into_float(self) -> Result<f64, ComponentBuildError>;
    fn into_string(self) -> Result<String, ComponentBuildError>;
    fn into_type<T: serde::de::DeserializeOwned>(self) -> Result<T, ComponentBuildError>;
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
}
