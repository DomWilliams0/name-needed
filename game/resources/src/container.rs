use crate::error::{ResourceError, ResourceErrorKind};
use common::*;
use memmap::Mmap;
use std::ffi::OsStr;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use walkdir::WalkDir;

/// Represents a directory
pub trait ResourceContainer {
    const DIR: &'static str;

    fn path(&self) -> &Path;
    fn component_offset(&self) -> usize;

    fn get_file(&self, file: impl AsRef<ResourceFile>) -> Result<ResourcePath, ResourceError> {
        let file = self.path().join(&file.as_ref().0);
        if !file.exists() {
            Err(ResourceError(file, ResourceErrorKind::FileNotFound))
        } else if !file.is_file() {
            Err(ResourceError(file, ResourceErrorKind::NotAFile))
        } else {
            Ok(ResourcePath(file, self.component_offset()))
        }
    }
}

/// A method of reading a file
pub trait ReadResource: Sized {
    fn read_resource(path: impl AsRef<ResourcePath>) -> Result<Self, ResourceError>;
}

/// A path to a resource, relative to the root resource path. Not identical to a file path (e.g. no
/// relative ../, no C:\\)
pub struct ResourcePath(PathBuf, usize);

/// A resource file name
#[repr(transparent)]
pub struct ResourceFile(OsStr);

impl ResourcePath {
    /// Path on disk, returns None if in-memory only
    pub fn file_path(&self) -> Option<&Path> {
        // TODO depends on feature gate
        Some(self.0.as_path())
    }

    pub fn resource_path(&self) -> PathBuf {
        self.0
            .components()
            .skip(self.1)
            .map(|c| c.as_os_str())
            .collect()
    }
}

impl AsRef<ResourcePath> for ResourcePath {
    fn as_ref(&self) -> &ResourcePath {
        self
    }
}

impl AsRef<ResourceFile> for ResourceFile {
    fn as_ref(&self) -> &ResourceFile {
        self
    }
}

impl AsRef<ResourceFile> for str {
    fn as_ref(&self) -> &ResourceFile {
        AsRef::<OsStr>::as_ref(self).as_ref()
    }
}

impl AsRef<ResourceFile> for OsStr {
    fn as_ref(&self) -> &ResourceFile {
        // safety: repr transparent
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl Debug for ResourceFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.0)
    }
}

impl Display for ResourcePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO add feature gate info e.g. from disk, from archive
        write!(f, "{}", self.0.display())
    }
}

pub fn recurse<R: ResourceContainer, T: ReadResource>(
    container: &R,
    ext: &'static str,
) -> impl Iterator<Item = Result<T, ResourceError>> {
    let ext = OsStr::new(ext);
    let offset = container.component_offset();
    WalkDir::new(container.path())
        .into_iter()
        .filter_map(move |e| match e {
            Err(e) => {
                warn!("failed to read resource file"; "error" => %e);
                None
            }
            Ok(e) => {
                if e.path().is_file()
                    && e.path()
                        .extension()
                        .map(|this_ext| this_ext == ext)
                        .unwrap_or(false)
                {
                    Some(ResourcePath(e.into_path(), offset))
                } else {
                    None
                }
            }
        })
        .map(T::read_resource)
}

impl ReadResource for (File, Mmap, Rc<Path>) {
    fn read_resource(path: impl AsRef<ResourcePath>) -> Result<Self, ResourceError> {
        let path = path.as_ref();
        let file = File::open(&path.0);
        file.and_then(|f| {
            let mapped = unsafe { Mmap::map(&f) };
            mapped.map(|m| (f, m, path.0.as_path().into())) // keep file alive
        })
        .map_err(|e| ResourceError(path.0.to_path_buf(), ResourceErrorKind::Io(Arc::new(e))))
    }
}

impl ReadResource for String {
    fn read_resource(path: impl AsRef<ResourcePath>) -> Result<Self, ResourceError> {
        let path = path.as_ref();
        std::fs::read_to_string(&path.0)
            .map_err(|e| ResourceError(path.0.to_path_buf(), ResourceErrorKind::Io(Arc::new(e))))
    }
}

/// Declares a directory but not where it is in the hierarchy (see [child])
#[macro_export]
macro_rules! resources {
    ($name:ident, $dir:expr) => {
        #[derive(Clone)]
        pub struct $name {
            path: PathBuf,
            component_offset: usize,
        }

        impl ResourceContainer for $name {
            const DIR: &'static str = $dir;

            #[inline]
            fn path(&self) -> &Path {
                &self.path
            }

            #[inline]
            fn component_offset(&self) -> usize {
                self.component_offset
            }
        }
    };
}

/// Declares a pre-declared directory to be a child of this one (this = the impl block this is in)
#[macro_export]
macro_rules! child {
    ($name:ident, $child:ident) => {
        pub fn $name(&self) -> Result<$child, ResourceError> {
            let path = get_dir(&self.path, $child::DIR)?;
            Ok($child {
                path,
                component_offset: self.component_offset,
            })
        }
    };
}
