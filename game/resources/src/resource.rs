use crate::container::ResourceContainer;
use crate::error::{ResourceError, ResourceErrorKind};
use std::path::{Path, PathBuf};

macro_rules! resources {
    ($name:ident, $dir:expr) => {
        #[derive(Clone)]
        pub struct $name(PathBuf);

        impl AsRef<Path> for $name {
            fn as_ref(&self) -> &Path {
                &*self.0
            }
        }

        impl From<PathBuf> for $name {
            fn from(path: PathBuf) -> Self {
                Self(path)
            }
        }

        impl ResourceContainer for $name {
            const DIR: &'static str = $dir;
        }
    };
}

macro_rules! child {
    ($name:ident, $child:ident) => {
        pub fn $name(&self) -> Result<$child, ResourceError> {
            get_dir(&self.0, <$child as ResourceContainer>::DIR).map($child)
        }
    };
}

resources!(Resources, "resources");

resources!(Definitions, "definitions");
resources!(WorldGen, "worldgen");
resources!(Shaders, "shaders");

impl Resources {
    pub fn new<P: AsRef<Path>>(game_dir: P) -> Result<Self, ResourceError> {
        get_dir(game_dir, "resources").map(Self)
    }

    child!(definitions, Definitions);
    child!(world_gen, WorldGen);
    child!(shaders, Shaders);
}

fn get_dir<R: AsRef<Path>, D: AsRef<Path>>(root: R, dir: D) -> Result<PathBuf, ResourceError> {
    let root = root.as_ref();
    let dir = dir.as_ref();

    let path = root.join(dir);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(ResourceError(
            root.to_owned(),
            ResourceErrorKind::MissingDirectory(
                dir.to_str()
                    .expect("expected path to be unicode")
                    .to_owned(),
            ),
        ))
    }
}
