mod builder;
mod loader;
mod registry;

pub use builder::{BuilderError, DefinitionBuilder, EntityPosition};
pub use loader::{load, Definition, ValueImpl};
pub use registry::Registry;

#[cfg(test)]
pub use loader::load_from_str;

use crate::definitions::loader::DefinitionSource;
use crate::ecs::ComponentBuildError;
use common::{
    derive_more::{Display, Error},
    *,
};

#[derive(Debug, Display, Error)]
#[display(fmt = "Error loading definition {:?}: {}", "_0", "_1")]
pub struct DefinitionError(pub DefinitionSource, pub DefinitionErrorKind);

#[derive(Debug, Display, Error)]
pub enum DefinitionErrorKind {
    #[display(fmt = "Failed to read definition file: {}", _0)]
    Resource(resources::ResourceErrorKind),

    #[display(fmt = "Bad format: {}", _0)]
    Format(ron::Error),

    #[display(fmt = "UID {:?} already registered", _0)]
    AlreadyRegistered(#[error(not(source))] String),

    #[display(fmt = "No such definition with UID {:?}", _0)]
    NoSuchDefinition(#[error(not(source))] String),

    #[display(fmt = "Invalid UID {:?}", _0)]
    InvalidUid(#[error(not(source))] String),

    #[display(fmt = "No such component type {:?}", _0)]
    NoSuchComponent(#[error(not(source))] String),

    // TODO include which key caused the problem
    #[display(fmt = "Failed to build component: {}", _0)]
    ComponentBuild(ComponentBuildError),

    #[display(fmt = "Invalid parent {:?}", _0)]
    InvalidParent(#[error(not(source))] String),

    #[display(fmt = "Parent relation {}->{} would cause a cycle", _0, _1)]
    CyclicParentRelation(String, String),

    #[display(fmt = "Duplicate uid {:?}", _0)]
    DuplicateUid(#[error(not(source))] String),

    #[display(fmt = "Duplicate component with type {:?}", _0)]
    DuplicateComponent(#[error(not(source))] String),
}

#[derive(Debug, Error)]
pub struct DefinitionErrors(#[error(not(source))] pub Vec<DefinitionError>);

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

impl From<ComponentBuildError> for DefinitionErrorKind {
    fn from(e: ComponentBuildError) -> Self {
        Self::ComponentBuild(e)
    }
}
