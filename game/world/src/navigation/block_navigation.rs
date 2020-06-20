//! Navigation inside an area

use petgraph::graphmap::DiGraphMap;

use unit::world::{BlockPosition, SlabIndex, SlabPosition};

use crate::navigation::astar::astar;
use crate::navigation::path::{BlockPath, BlockPathNode};
use crate::navigation::EdgeCost;

type BlockNavGraph = DiGraphMap<BlockNavNode, BlockNavEdge>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Ord, PartialOrd, Hash)]
pub struct BlockNavNode(pub BlockPosition);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BlockNavEdge(pub EdgeCost);

#[cfg_attr(test, derive(Clone))]
pub struct BlockGraph {
    graph: BlockNavGraph,
}

#[derive(Debug, Clone)]
pub enum BlockPathError {
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
        use common::Itertools;

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
    ) -> Result<BlockPath, BlockPathError> {
        let from = from.into();
        let to = to.into();

        let src = BlockNavNode(from);
        let dst = BlockNavNode(to);

        let path = astar(
            &self.graph,
            src,
            |n| n == dst,
            |(_, _, e)| e.0.weight(),
            |n| {
                // manhattan distance
                let [nx, ny, nz]: [i32; 3] = n.0.into();
                let [gx, gy, gz]: [i32; 3] = dst.0.into();
                // TODO use vertical distance differently?

                let dx = (nx - gx).abs();
                let dy = (ny - gy).abs();
                let dz = (nz - gz).abs();
                (dx + dy + dz) as f32
            },
        )
        .ok_or_else(|| BlockPathError::NoPath(to, from))?;

        // TODO reuse vec allocation
        let mut out_path = Vec::with_capacity(path.len());

        for (_, (from, to)) in path {
            let edge = self.graph.edge_weight(from, to).unwrap();
            out_path.push(BlockPathNode {
                block: from.0,
                exit_cost: edge.0,
            });
        }

        Ok(BlockPath(out_path))
    }
}

#[cfg(test)]
mod tests {
    use unit::world::ChunkPosition;

    use crate::block::BlockType;
    use crate::navigation::{BlockPathNode, WorldArea};
    use crate::world::helpers::world_from_chunks;
    use crate::{ChunkBuilder, EdgeCost};

    #[test]
    fn simple_path() {
        let world = world_from_chunks(vec![ChunkBuilder::new()
            .fill_slice(1, BlockType::Stone)
            .set_block((5, 5, 2), BlockType::Grass)
            .set_block((6, 5, 3), BlockType::Grass)
            .build((0, 0))])
        .into_inner();
        let chunk = world.find_chunk_with_pos(ChunkPosition(0, 0)).unwrap();
        let graph = chunk.block_graph_for_area(WorldArea::new((0, 0))).unwrap();

        let path = graph
            .find_block_path((3, 5, 2), (6, 5, 4))
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

        assert_eq!(path.0, expected);

        // in reverse
        let path = graph
            .find_block_path((6, 5, 4), (3, 5, 2))
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

        assert_eq!(path.0, expected);
    }
}
