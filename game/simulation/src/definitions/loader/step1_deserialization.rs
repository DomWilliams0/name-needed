use std::cell::{Ref, RefCell, RefMut};
use std::fs::File;
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;

use serde::de::Error;
use serde::Deserialize;

use common::derive_more::IntoIterator;
use common::*;
use resources::ResourceError;

use crate::definitions::loader::step2_preprocessing::ProcessedComponents;
use crate::definitions::{DefinitionError, DefinitionErrorKind};
use crate::ecs;
use crate::ecs::ComponentBuildError;

/// Parent hierarchy is unprocessed
#[derive(Debug, Deserialize)]
pub struct DeserializedDefinition {
    uid: String,

    #[serde(default)]
    parent: String,

    #[serde(default)]
    category: String,

    #[serde(default)]
    r#abstract: bool,

    components: Components,

    // Populated by preprocessing
    #[serde(skip)]
    processed_components: RefCell<ProcessedComponents>,

    #[serde(skip)]
    source: DefinitionSource,
}

#[derive(Debug, Clone)]
pub enum DefinitionSource {
    File(Rc<Path>),
    Memory,
}

#[derive(Debug, Deserialize, IntoIterator, Clone, Default)]
#[serde(transparent)]
pub struct Components(Vec<ron::Value>);

impl Deref for Components {
    type Target = Vec<ron::Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DeserializedDefinition {
    pub fn parent(&self) -> Option<&str> {
        if self.parent.is_empty() {
            None
        } else {
            Some(&self.parent)
        }
    }

    pub fn uid(&self) -> &str {
        &self.uid
    }

    pub fn source(&self) -> DefinitionSource {
        self.source.clone()
    }

    pub fn is_abstract(&self) -> bool {
        self.r#abstract
    }

    pub fn make_error(&self, uid: Option<String>, e: DefinitionErrorKind) -> DefinitionError {
        DefinitionError {
            uid,
            src: self.source(),
            kind: e,
        }
    }

    pub fn validate_and_process_components(
        &mut self,
    ) -> Result<(), Vec<(String, DefinitionErrorKind)>> {
        // validation
        let mut errors = Vec::new();
        if self.uid.is_empty()
            || !self
                .uid
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
        {
            errors.push((self.uid.clone(), DefinitionErrorKind::InvalidUid));
        }

        let components = std::mem::take(&mut self.components);
        match components.into_processed().map(RefCell::new) {
            Err(errs) => {
                errors.extend(errs.into_iter().map(|err| (self.uid.clone(), err)));
            }
            Ok(comps) => {
                self.processed_components = comps;
            }
        }

        if !errors.is_empty() {
            Err(errors)
        } else {
            Ok(())
        }
    }

    pub fn processed_components(&self) -> Ref<ProcessedComponents> {
        self.processed_components.borrow()
    }
    pub fn processed_components_mut(&self) -> RefMut<ProcessedComponents> {
        self.processed_components.borrow_mut()
    }

    pub fn from_ron(bytes: &[u8], source: DefinitionSource) -> Result<Vec<Self>, DefinitionError> {
        ron::de::from_bytes(bytes)
            .map(|mut vec: Vec<DeserializedDefinition>| {
                for def in vec.iter_mut() {
                    def.source = source.clone();
                }
                vec
            })
            .map_err(|e| DefinitionError {
                uid: None,
                src: source.clone(),
                kind: DefinitionErrorKind::Format(e),
            })
    }

    pub fn into_inner(
        self,
    ) -> (
        String,
        DefinitionSource,
        Option<String>,
        ProcessedComponents,
    ) {
        (
            self.uid,
            self.source,
            if self.category.is_empty() {
                None
            } else {
                Some(self.category)
            },
            self.processed_components.into_inner(),
        )
    }
}

pub fn collect_raw_definitions(
    resources: resources::Definitions,
) -> (Vec<DeserializedDefinition>, Vec<DefinitionError>) {
    let mut definitions = Vec::with_capacity(512);
    let mut errors = Vec::new();

    // collect unprocessed definitions
    for file in resources::recurse::<_, (File, resources::Mmap, Rc<Path>)>(&resources, "ron") {
        // handle resource error
        let file = file.map_err(|ResourceError(path, e)| DefinitionError {
            uid: None,
            src: DefinitionSource::File(path.into()),
            kind: DefinitionErrorKind::Resource(e),
        });

        // deserialize
        let definition = file.and_then(|(_, mapped, path)| {
            debug!("loading definitions"; "path" => path.display());
            DeserializedDefinition::from_ron(&*mapped, DefinitionSource::File(path))
        });

        match definition {
            Ok(loaded) => {
                debug!("loaded {count}", count = loaded.len());
                definitions.extend(loaded.into_iter());
            }
            Err(e) => {
                debug!("failed to deserialize definitions"; "error" => %e);
                errors.push(e)
            }
        }
    }

    (definitions, errors)
}
impl ecs::Value for ron::Value {
    fn into_bool(self) -> Result<bool, ComponentBuildError> {
        self.into_type()
    }

    fn into_int(self) -> Result<i64, ComponentBuildError> {
        self.into_type()
    }

    fn into_float(self) -> Result<f64, ComponentBuildError> {
        self.into_type()
    }

    fn into_string(self) -> Result<String, ComponentBuildError> {
        self.into_type()
    }

    fn into_unit(self) -> Result<(), ComponentBuildError> {
        self.into_type()
    }

    fn as_unit(&self) -> Result<(), ComponentBuildError> {
        if let ron::Value::Unit = self {
            Ok(())
        } else {
            Err(ComponentBuildError::Deserialize(ron::Error::custom(
                "not a unit type",
            )))
        }
    }

    fn as_int(&self) -> Result<i64, ComponentBuildError> {
        if let ron::Value::Number(ron::Number::Integer(i)) = self {
            Ok(*i)
        } else {
            Err(ComponentBuildError::Deserialize(ron::Error::custom(
                "not an integer",
            )))
        }
    }

    fn into_type<T: serde::de::DeserializeOwned>(self) -> Result<T, ComponentBuildError> {
        self.into_rust().map_err(ComponentBuildError::Deserialize)
    }
}

impl Default for DefinitionSource {
    fn default() -> Self {
        Self::Memory
    }
}

impl Display for DefinitionSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DefinitionSource::File(path) => Display::fmt(&path.display(), f),
            DefinitionSource::Memory => write!(f, "in-memory buffer"),
        }
    }
}
