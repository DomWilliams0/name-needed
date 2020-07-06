use unit::world::{BlockPosition, WorldPosition};

use crate::navigation::{AreaNavEdge, AreaPathError, BlockPathError, EdgeCost, WorldArea};

// TODO smallvecs

// TODO derive(Error) for NavigationError
#[derive(Debug, Clone)]
pub enum NavigationError {
    SourceNotWalkable(WorldPosition),
    TargetNotWalkable(WorldPosition),
    NoSuchArea(WorldArea),
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

#[derive(Debug)]
pub struct BlockPath {
    /// Doesnt include target node
    pub path: Vec<BlockPathNode>,

    /// The actual target, might be different from the requested because of `SearchGoal`
    pub target: BlockPosition,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub(crate) struct AreaPathNode {
    pub area: WorldArea,
    /// None for first node
    pub entry: Option<AreaNavEdge>,
}

#[derive(Copy, Clone)]
pub enum SearchGoal {
    /// Arrive exactly at the target
    Arrive,

    /// Arrive within 1 block of the target
    Adjacent,
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

impl WorldPath {
    pub fn new(path: Vec<WorldPathNode>, target: WorldPosition) -> Self {
        Self { path, target }
    }

    pub fn path(&self) -> &[WorldPathNode] {
        &self.path
    }

    pub const fn target(&self) -> WorldPosition {
        self.target
    }
}
