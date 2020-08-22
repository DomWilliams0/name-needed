use std::cell::{Ref, RefCell, RefMut};
use std::fs::File;
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;

use serde::Deserialize;

use common::derive_more::IntoIterator;
use common::*;
use resources::{resource, ResourceError};

use crate::definitions::loader::step2_preprocessing::ProcessedComponents;
use crate::definitions::{DefinitionError, DefinitionErrorKind};
use crate::ecs;
use crate::ecs::ComponentBuildError;

pub type DefinitionUid = String;

/// Parent hierarchy is unprocessed
#[derive(Debug, Deserialize)]
pub struct DeserializedDefinition {
    uid: String,

    #[serde(default)]
    parent: String,

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
    File(Rc<PathBuf>),
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

    pub fn make_error(&self, e: DefinitionErrorKind) -> DefinitionError {
        DefinitionError(self.source(), e)
    }

    pub fn validate_and_process_components(&mut self) -> Result<(), Vec<DefinitionErrorKind>> {
        // validation
        let mut errors = Vec::new();
        if self.uid.is_empty() || !self.uid.chars().all(|c| c.is_alphanumeric() || c == '_') {
            errors.push(DefinitionErrorKind::InvalidUid(self.uid.clone()));
        }

        let components = std::mem::take(&mut self.components);
        match components.into_processed().map(RefCell::new) {
            Err(errs) => {
                errors.extend(errs.into_iter());
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
            .map_err(|e| DefinitionError(source.clone(), DefinitionErrorKind::Format(e)))
    }

    pub fn into_inner(self) -> (DefinitionUid, DefinitionSource, ProcessedComponents) {
        (
            self.uid,
            self.source,
            self.processed_components.into_inner(),
        )
    }
}

pub fn collect_raw_definitions(
    resources: resource::Definitions,
) -> (Vec<DeserializedDefinition>, Vec<DefinitionError>) {
    let mut definitions = Vec::with_capacity(512);
    let mut errors = Vec::new();

    // collect unprocessed definitions
    for file in resources::recurse::<_, (File, resources::Mmap, Rc<PathBuf>)>(&resources, "ron") {
        // handle resource error
        let file = file.map_err(|ResourceError(path, e)| {
            DefinitionError(
                DefinitionSource::File(Rc::new(path)),
                DefinitionErrorKind::Resource(e),
            )
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
                debug!("failed to load definitions"; "error" => %e);
                errors.push(e)
            }
        }
    }

    (definitions, errors)
}
impl ecs::Value for ron::Value {
    fn into_int(self) -> Result<i64, ComponentBuildError> {
        self.into_type()
    }

    fn into_float(self) -> Result<f64, ComponentBuildError> {
        self.into_type()
    }

    fn into_string(self) -> Result<String, ComponentBuildError> {
        self.into_type()
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
