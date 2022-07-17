use misc::*;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Error)]
#[error("Error loading resource from {0:?}: {1}")]
pub struct ResourceError(pub PathBuf, #[source] pub ResourceErrorKind);

#[derive(Debug, Error, Clone)]
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
    Io(#[source] Arc<std::io::Error>), // Arc for cloning...
}
