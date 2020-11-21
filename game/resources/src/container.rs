use crate::error::{ResourceError, ResourceErrorKind};
use common::*;
use memmap::Mmap;
use std::ffi::OsStr;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use walkdir::WalkDir;

pub trait ResourceContainer: AsRef<Path> + From<PathBuf> {
    const DIR: &'static str;

    fn get_file<P: AsRef<Path>>(&self, name: P) -> Result<PathBuf, ResourceError> {
        let file = self.as_ref().join(name.as_ref());
        if file.is_file() {
            Ok(file)
        } else {
            Err(ResourceError(file, ResourceErrorKind::NotAFile))
        }
    }
}

pub trait ReadResource: Sized {
    fn read(path: PathBuf) -> Result<Self, ResourceError>;
}

pub fn recurse<R: ResourceContainer, T: ReadResource>(
    container: &R,
    ext: &'static str,
) -> impl Iterator<Item = Result<T, ResourceError>> {
    let ext = OsStr::new(ext);
    WalkDir::new(container.as_ref())
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
                    Some(e.into_path())
                } else {
                    None
                }
            }
        })
        .map(T::read)
}

impl ReadResource for PathBuf {
    fn read(path: PathBuf) -> Result<Self, ResourceError> {
        Ok(path)
    }
}

impl ReadResource for (File, Mmap, Rc<Path>) {
    fn read(path: PathBuf) -> Result<Self, ResourceError> {
        let file = File::open(&path);
        file.and_then(|f| {
            let mapped = unsafe { Mmap::map(&f) };
            mapped.map(|m| (f, m, path.clone().into())) // keep file alive
        })
        .map_err(|e| ResourceError(path, ResourceErrorKind::Io(e)))
    }
}

impl ReadResource for String {
    fn read(path: PathBuf) -> Result<Self, ResourceError> {
        std::fs::read_to_string(&path).map_err(|e| ResourceError(path, ResourceErrorKind::Io(e)))
    }
}
