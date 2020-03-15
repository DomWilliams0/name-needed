use std::fmt::{Display, Error, Formatter};

use crate::area::{EdgeCost, WorldArea};
use unit::world::{BlockPosition, WorldPosition};

#[derive(Debug)]
pub(crate) struct BlockPath(Vec<(BlockPosition, EdgeCost)>);

impl Display for BlockPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "BlockPath({:?})", self.0)
    }
}

impl BlockPath {
    pub fn new(points: Vec<(BlockPosition, EdgeCost)>) -> Self {
        Self(points)
    }

    pub fn into_iter(self) -> impl Iterator<Item = (BlockPosition, EdgeCost)> {
        // skip the first node which is just the start
        self.0.into_iter().skip(1)
    }

    /// for easy comparisons in tests
    #[cfg(test)]
    pub fn as_tuples(&self) -> Vec<(u16, u16, i32)> {
        self.0.iter().map(|&p| p.0.into()).collect()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AreaPathNode {
    pub area: WorldArea,

    /// None for first area
    pub entry: Option<(WorldPosition, EdgeCost)>,

    /// None for last area
    pub exit: Option<(WorldPosition, EdgeCost)>,
}

#[derive(Debug)]
pub(crate) struct AreaPath(pub Vec<AreaPathNode>);

#[derive(Debug)]
pub struct WorldPath(pub Vec<(WorldPosition, EdgeCost)>);

pub type WorldPathSlice<'a> = &'a [(WorldPosition, EdgeCost)];

// ----

impl IntoIterator for AreaPath {
    type Item = AreaPathNode;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl IntoIterator for WorldPath {
    type Item = (WorldPosition, EdgeCost);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
