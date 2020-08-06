use common::derive_more::{Display, Error};
use common::*;

use std::path::PathBuf;

#[derive(Debug, Display, Error)]
#[display(fmt = "Error loading resource from {:?}: {}", "_0", "_1")]
pub struct ResourceError(pub PathBuf, pub ResourceErrorKind);

#[derive(Debug, Display, Error)]
pub enum ResourceErrorKind {
    #[display(fmt = "No such directory {:?}", "_0")]
    MissingDirectory(#[error(not(source))] String),

    #[display(fmt = "Path is not a file")]
    NotAFile,

    #[display(fmt = "Failed to read resource")]
    Io(std::io::Error),
}
