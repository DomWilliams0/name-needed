mod builder;
mod component;
mod loader;
mod registry;

pub use builder::{BuilderError, DefinitionBuilder, EntityPosition};
pub use component::DefinitionNameComponent;
pub use loader::{load, Definition, ValueImpl};
pub use registry::DefinitionRegistry;

#[cfg(test)]
pub use loader::load_from_str;

use crate::definitions::loader::DefinitionSource;
use crate::ecs::ComponentBuildError;
use common::*;

#[derive(Debug)]
pub struct DefinitionError {
    pub uid: Option<String>,
    pub src: DefinitionSource,
    pub kind: DefinitionErrorKind,
}

#[derive(Debug, Error, Clone)]
pub enum DefinitionErrorKind {
    #[error("Failed to read definition file: {0}")]
    Resource(#[from] resources::ResourceErrorKind),

    #[error("Bad format: {0}")]
    Format(#[from] ron::Error),

    #[error("UID {0:?} already registered")]
    AlreadyRegistered(String),

    #[error("No such definition with UID {0:?}")]
    NoSuchDefinition(String),

    #[error("Invalid UID")]
    InvalidUid,

    #[error("No such component type {0:?}")]
    NoSuchComponent(String),

    // TODO include which key caused the problem
    #[error("Failed to build component: {0}")]
    ComponentBuild(#[from] ComponentBuildError),

    #[error("Invalid parent {0:?}")]
    InvalidParent(String),

    #[error("Parent relation {0}->{1} would cause a cycle")]
    CyclicParentRelation(String, String),

    #[error("Duplicate uid")]
    DuplicateUid,

    #[error("Duplicate component with type {0:?}")]
    DuplicateComponent(String),
}

#[derive(Debug, Error)]
pub struct DefinitionErrors(pub Vec<DefinitionError>);

impl Display for DefinitionErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} errors: [", self.0.len())?;
        for (i, e) in self.0.iter().enumerate() {
            let comma = if i == 0 { "" } else { ", " };
            write!(f, "{}{}", comma, e)?;
        }

        write!(f, "]")
    }
}

impl Display for DefinitionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Error loading definition")?;

        if let Some(uid) = self.uid.as_ref() {
            write!(f, " '{uid}'")?;
        }

        write!(f, " in {}: {}", self.src, self.kind)
    }
}
