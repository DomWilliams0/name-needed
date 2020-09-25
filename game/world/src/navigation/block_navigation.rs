//! Navigation inside an area
use common::{
    derive_more::{Display, Error},
    *,
};

use petgraph::graphmap::DiGraphMap;
use petgraph::prelude::EdgeRef;

use unit::world::{BlockPosition, SlabIndex, SlabPosition};

use crate::navigation::astar::astar;
use crate::navigation::path::{BlockPath, BlockPathNode};
use crate::navigation::{EdgeCost, SearchGoal};

type BlockNavGraph = DiGraphMap<BlockNavNode, BlockNavEdge>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Ord, PartialOrd, Hash)]
pub struct BlockNavNode(pub BlockPosition);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BlockNavEdge(pub EdgeCost);

#[cfg_attr(test, derive(Clone))]
pub struct BlockGraph {
    graph: BlockNavGraph,
}

#[derive(Debug, Clone, Display, Error)]
pub enum BlockPathError {
    #[display(fmt = "No path found from {} to {}", _0, _1)]
    NoPath(BlockPosition, BlockPosition),
}

impl BlockGraph {
    pub fn new() -> Self {
        Self {
            graph: BlockNavGraph::new(),
        }
    }

    pub fn add_edge<F, T>(&mut self, from: F, to: T, cost: EdgeCost, slab: SlabIndex)
    where
        F: Into<SlabPosition>,
        T: Into<SlabPosition>,
    {
        let from = BlockNavNode(from.into().to_block_position(slab));
        let to = BlockNavNode(to.into().to_block_position(slab));

        self.graph.add_edge(from, to, BlockNavEdge(cost));
        self.graph.add_edge(to, from, BlockNavEdge(cost.opposite()));
    }

    #[cfg(test)]
    pub fn edges(&self, block: BlockPosition) -> Vec<(BlockPosition, EdgeCost)> {
        let node = BlockNavNode(block);
        let mut edges = self
            .graph
            .edges(node)
            .map(|(_, to, e)| (to.0, e.0))
            .collect_vec();

        edges.sort_unstable_by_key(|(pos, _)| *pos);
        edges
    }

    pub(crate) fn find_block_path<F: Into<BlockPosition>, T: Into<BlockPosition>>(
        &self,
        from: F,
        to: T,
        goal: SearchGoal,
    ) -> Result<BlockPath, BlockPathError> {
        let from = from.into();
        let to = to.into();

        // same source and dest is a success, if not a pointless one
        if from == to {
            debug!("pointless block path to same block"; "pos" => ?from);
            return Ok(BlockPath {
                path: vec![],
                target: to,
            });
        }

        fn manhattan(a: &BlockPosition, b: &BlockPosition) -> i32 {
            let [nx, ny, nz]: [i32; 3] = (*a).into();
            let [gx, gy, gz]: [i32; 3] = (*b).into();

            // TODO use vertical distance differently?

            let dx = (nx - gx).abs();
            let dy = (ny - gy).abs();
            let dz = (nz - gz).abs();
            dx + dy + dz
        }

        let src = BlockNavNode(from);
        let dst = BlockNavNode(to);

        let is_goal: Box<dyn FnMut(BlockNavNode) -> bool> = match goal {
            SearchGoal::Arrive => Box::new(|n| n == dst),
            SearchGoal::Adjacent => {
                Box::new(|n: BlockNavNode| self.graph.edges(n).any(|e| e.target() == dst))
            }
            SearchGoal::Nearby(range) => {
                Box::new(move |n: BlockNavNode| manhattan(&n.0, &dst.0) <= range.into())
            }
        };

        let path = astar(
            &self.graph,
            src,
            is_goal,
            |(_, _, e)| e.0.weight(),
            |n| manhattan(&n.0, &dst.0) as f32,
        )
        .ok_or_else(|| BlockPathError::NoPath(to, from))?;

        // TODO reuse vec allocation
        let mut out_path = Vec::with_capacity(path.len());

        let target = path.last().map(|(_, (_, target))| target).unwrap_or(&dst).0;

        for (_, (from, to)) in path {
            let edge = self.graph.edge_weight(from, to).unwrap();
            out_path.push(BlockPathNode {
                block: from.0,
                exit_cost: edge.0,
            });
        }

        Ok(BlockPath {
            path: out_path,
            target,
        })
    }
}

#[cfg(test)]
mod tests {
    use unit::world::ChunkPosition;

    use crate::block::BlockType;
    use crate::navigation::{BlockPathNode, SearchGoal, WorldArea};
    use crate::world::helpers::world_from_chunks_blocking;
    use crate::{ChunkBuilder, EdgeCost};

    #[test]
    fn simple_path() {
        let world = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(1, BlockType::Stone)
            .set_block((5, 5, 2), BlockType::Grass)
            .set_block((6, 5, 3), BlockType::Grass)
            .build((0, 0))])
        .into_inner();
        let chunk = world.find_chunk_with_pos(ChunkPosition(0, 0)).unwrap();
        let graph = chunk.block_graph_for_area(WorldArea::new((0, 0))).unwrap();

        let path = graph
            .find_block_path((3, 5, 2), (6, 5, 4), SearchGoal::Arrive)
            .expect("path should succeed");
        let expected = vec![
            BlockPathNode {
                block: (3, 5, 2).into(),
                exit_cost: EdgeCost::Walk,
            },
            BlockPathNode {
                block: (4, 5, 2).into(),
                exit_cost: EdgeCost::JumpUp,
            },
            BlockPathNode {
                block: (5, 5, 3).into(),
                exit_cost: EdgeCost::JumpUp,
            },
            // goal omitted
        ];

        assert_eq!(path.path, expected);

        // in reverse
        let path = graph
            .find_block_path((6, 5, 4), (3, 5, 2), SearchGoal::Arrive)
            .expect("reverse path should succeed");

        let expected = vec![
            BlockPathNode {
                block: (6, 5, 4).into(),
                exit_cost: EdgeCost::JumpDown,
            },
            BlockPathNode {
                block: (5, 5, 3).into(),
                exit_cost: EdgeCost::JumpDown,
            },
            BlockPathNode {
                block: (4, 5, 2).into(),
                exit_cost: EdgeCost::Walk,
            },
            // goal omitted
        ];

        assert_eq!(path.path, expected);
    }

    #[test]
    fn no_hippity_hoppity() {
        // regression test for a bug where an invalid path through the back of a staircase is generated.
        // dont bang ur head with ur hoppity

        let world = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(1, BlockType::Stone)
            .set_block((3, 3, 2), BlockType::Stone) // step up
            .set_block((2, 3, 3), BlockType::Stone) // step up from the step up
            .build((0, 0))])
        .into_inner();

        let start = (2, 3, 2); // underneath the second step
        let end = (5, 3, 2); // walk around the first step to here pls

        let path = {
            let chunk = world.find_chunk_with_pos(ChunkPosition(0, 0)).unwrap();
            let graph = chunk.block_graph_for_area(WorldArea::new((0, 0))).unwrap();
            graph
                .find_block_path(start, end, SearchGoal::Arrive)
                .expect("path should succeed")
        };

        // there should be no jumps in this nice easy path around the staircase
        let not_walks = path
            .path
            .iter()
            .filter(|node| node.exit_cost != EdgeCost::Walk)
            .count();

        assert_eq!(not_walks, 0);
    }
}
