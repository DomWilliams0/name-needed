use std::fmt::{Display, Error, Formatter};

use unit::world::{BlockPosition, WorldPosition};

use crate::navigation::{AreaNavEdge, AreaPathError, BlockPathError, EdgeCost, WorldArea};

// TODO smallvecs

#[derive(Debug, Clone)]
pub enum NavigationError {
    SourceNotWalkable(WorldPosition),
    TargetNotWalkable(WorldPosition),
    NoSuchArea(WorldArea),
    ZeroLengthPath,
    AreaError(AreaPathError),
    BlockError(WorldArea, BlockPathError),
}

impl From<AreaPathError> for NavigationError {
    fn from(e: AreaPathError) -> Self {
        NavigationError::AreaError(e)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct BlockPathNode {
    pub block: BlockPosition,
    pub exit_cost: EdgeCost,
}

/// Doesnt include goal node
#[derive(Debug)]
pub struct BlockPath(pub Vec<BlockPathNode>);

impl Display for BlockPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "BlockPath({:?})", self.0)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub(crate) struct AreaPathNode {
    pub area: WorldArea,
    /// None for first node
    pub entry: Option<AreaNavEdge>,
}

impl AreaPathNode {
    pub fn new_start(area: WorldArea) -> Self {
        Self { area, entry: None }
    }
    pub fn new(area: WorldArea, entry: AreaNavEdge) -> Self {
        Self {
            area,
            entry: Some(entry),
        }
    }
}

#[derive(Debug)]
pub struct AreaPath(pub(crate) Vec<AreaPathNode>);

#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct WorldPathNode {
    pub block: WorldPosition,
    pub exit_cost: EdgeCost,
}

#[derive(Debug)]
pub struct WorldPath {
    path: Vec<WorldPathNode>,
    target: WorldPosition,
}

impl WorldPath {
    pub fn new(path: Vec<WorldPathNode>, target: WorldPosition) -> Self {
        Self { path, target }
    }

    pub fn path(&self) -> &[WorldPathNode] {
        &self.path
    }

    pub fn target(&self) -> &WorldPosition {
        &self.target
    }
}

// TODO
pub type WorldPathSlice<'a> = &'a [(WorldPosition, EdgeCost)];
