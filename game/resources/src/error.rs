use common::*;

use std::path::PathBuf;

#[derive(Debug, Error)]
#[error("Error loading resource from {0:?}: {1}")]
pub struct ResourceError(pub PathBuf, #[source] pub ResourceErrorKind);

#[derive(Debug, Error)]
pub enum ResourceErrorKind {
    #[error("No such directory {0}")]
    MissingDirectory(PathBuf),

    #[error("File not found")]
    FileNotFound,

    #[error("Path is not a file")]
    NotAFile,

    #[error("Resource path is invalid")]
    InvalidPath,

    #[error("Failed to read resource: {0}")]
    Io(#[from] std::io::Error),
}
