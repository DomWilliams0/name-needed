pub use petgraph::prelude::NodeIndex;
use petgraph::{Graph, Undirected};

use crate::block::BlockHeight;
use crate::BlockPosition;

pub type NavIdx = u32;
pub type NavGraph = Graph<Node, Edge, Undirected, NavIdx>; // TODO directed

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Node(pub BlockPosition);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Edge {
    /// 1 high jump
    Jump,

    /// Walk along a block of the given height
    Walk(BlockHeight),
}

impl Edge {
    pub fn weight(self) -> i32 {
        // TODO currently arbitrary, should depend on physical attributes
        match self {
            Edge::Jump => 5,
            Edge::Walk(BlockHeight::Full) => 1,
            Edge::Walk(BlockHeight::Half) => 2,
        }
    }
}
