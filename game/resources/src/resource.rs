//! Resource filesystem structure declaration for the game

use crate::container::ResourceContainer;
use crate::error::{ResourceError, ResourceErrorKind};
use crate::{child, resources};
use std::path::{Path, PathBuf};

resources!(Resources, "resources");

resources!(Definitions, "definitions");
resources!(WorldGen, "worldgen");
resources!(Shaders, "shaders");

impl Resources {
    pub fn new(game_dir: impl AsRef<Path>) -> Result<Self, ResourceError> {
        let game_dir = game_dir.as_ref();
        let path = get_dir(game_dir, "resources")?;
        let component_offset = path.components().count();
        Ok(Self {
            path,
            component_offset,
        })
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
            ResourceErrorKind::MissingDirectory(dir.to_owned()),
        ))
    }
}
