//! Navigation inside an area

use petgraph::graphmap::DiGraphMap;
use petgraph::prelude::EdgeRef;
use petgraph::visit::Visitable;

use common::*;
use unit::world::{BlockPosition, ChunkLocation, SlabIndex, SlabPosition};

use crate::navigation::path::{BlockPath, BlockPathNode};
use crate::navigation::search::{self, ExploreResult, SearchContext};
use crate::navigation::{EdgeCost, SearchGoal};
use crate::{ExplorationFilter, ExplorationResult};

type BlockNavGraph = DiGraphMap<BlockNavNode, BlockNavEdge>;
pub type BlockGraphSearchContext = SearchContext<
    BlockNavNode,
    (BlockNavNode, BlockNavNode),
    f32,
    <BlockNavGraph as Visitable>::Map,
>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Ord, PartialOrd, Hash)]
pub struct BlockNavNode(pub BlockPosition);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BlockNavEdge(pub EdgeCost);

#[cfg_attr(test, derive(Clone))]
pub struct BlockGraph {
    graph: BlockNavGraph,
}

#[derive(Debug, Clone, Error)]
pub enum BlockPathError {
    #[error("No path found from {0} to {1}")]
    NoPath(BlockPosition, BlockPosition),
}

impl BlockGraph {
    pub fn new() -> Self {
        Self {
            graph: BlockNavGraph::new(),
        }
    }

    pub fn search_context() -> BlockGraphSearchContext {
        BlockGraphSearchContext::new::<BlockNavGraph>()
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

    pub(crate) fn find_block_path(
        &self,
        from: BlockPosition,
        to: BlockPosition,
        goal: SearchGoal,
        context: &BlockGraphSearchContext,
    ) -> Result<BlockPath, BlockPathError> {
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

        let heuristic: Box<dyn FnMut(BlockNavNode) -> f32> = match goal {
            SearchGoal::Nearby(range) => {
                let range = range as f32;
                Box::new(move |n| (manhattan(&n.0, &dst.0) as f32 - range).max(0.0))
            }
            _ => Box::new(|n| manhattan(&n.0, &dst.0) as f32),
        };

        let is_goal: Box<dyn FnMut(BlockNavNode) -> bool> = match goal {
            SearchGoal::Arrive => Box::new(|n| n == dst),
            SearchGoal::Adjacent => {
                Box::new(|n: BlockNavNode| self.graph.edges(n).any(|e| e.target() == dst))
            }
            SearchGoal::Nearby(range) => {
                Box::new(move |n: BlockNavNode| n != src && manhattan(&n.0, &dst.0) <= range.into())
            }
        };

        search::astar(
            &self.graph,
            src,
            is_goal,
            |(_, _, e)| e.0.weight(),
            heuristic,
            context,
        );

        self.block_path_from_search_result(context)
            .ok_or(BlockPathError::NoPath(to, from))
    }

    /// Uses as much fuel as possible to find a reachable block
    pub(crate) fn explore(
        &self,
        from: BlockPosition,
        fuel: &mut u32,
        context: &BlockGraphSearchContext,
        random: impl Rng,
        filter: Option<(&ExplorationFilter, ChunkLocation)>,
    ) -> (ExploreResult, Option<BlockPosition>) {
        let src = BlockNavNode(from);

        let filter = move |node: BlockNavNode| {
            filter
                .map(
                    |(func, this_chunk)| match func.0(node.0.to_world_position(this_chunk)) {
                        ExplorationResult::Abort => true,
                        ExplorationResult::Continue => false,
                    },
                )
                .unwrap_or_default()
        };
        let result = search::explore(
            &self.graph,
            src,
            fuel,
            |n| n.0.is_edge(),
            context,
            random,
            filter,
        );

        let path = &*context.result();
        (result, path.last().map(|(_, (_, target))| target.0))
    }

    /// None if empty
    fn block_path_from_search_result(
        &self,
        context: &BlockGraphSearchContext,
    ) -> Option<BlockPath> {
        let path = &*context.result();
        if path.is_empty() {
            return None;
        };

        // TODO improve allocations
        let mut out_path = Vec::with_capacity(path.len());

        let target = path.last().map(|(_, (_, target))| target.0).unwrap(); // not empty

        for &(_, (from, to)) in path.iter() {
            let edge = self.graph.edge_weight(from, to).unwrap();
            out_path.push(BlockPathNode {
                block: from.0,
                exit_cost: edge.0,
            });
        }
        Some(BlockPath {
            path: out_path,
            target,
        })
    }

    /// (edges, nodes)
    pub fn len(&self) -> (usize, usize) {
        let edges = self.graph.edge_count();
        let nodes = self.graph.node_count();
        (edges, nodes)
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use unit::world::ChunkLocation;

    use crate::helpers::DummyBlockType;
    use crate::navigation::{BlockGraph, BlockPathNode, SearchGoal, WorldArea};
    use crate::world::helpers::world_from_chunks_blocking;
    use crate::{ChunkBuilder, EdgeCost};

    #[test]
    fn simple_path() {
        let world = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(1, DummyBlockType::Stone)
            .set_block((5, 5, 2), DummyBlockType::Grass)
            .set_block((6, 5, 3), DummyBlockType::Grass)
            .build((0, 0))])
        .into_inner();
        let chunk = world.find_chunk_with_pos(ChunkLocation(0, 0)).unwrap();
        let graph = chunk.block_graph_for_area(WorldArea::new((0, 0))).unwrap();

        let path = graph
            .find_block_path(
                (3, 5, 2).try_into().unwrap(),
                (6, 5, 4).try_into().unwrap(),
                SearchGoal::Arrive,
                &BlockGraph::search_context(),
            )
            .expect("path should succeed");
        let expected = vec![
            BlockPathNode {
                block: (3, 5, 2).try_into().unwrap(),
                exit_cost: EdgeCost::Walk,
            },
            BlockPathNode {
                block: (4, 5, 2).try_into().unwrap(),
                exit_cost: EdgeCost::JumpUp,
            },
            BlockPathNode {
                block: (5, 5, 3).try_into().unwrap(),
                exit_cost: EdgeCost::JumpUp,
            },
            // goal omitted
        ];

        assert_eq!(path.path, expected);

        // in reverse
        let path = graph
            .find_block_path(
                (6, 5, 4).try_into().unwrap(),
                (3, 5, 2).try_into().unwrap(),
                SearchGoal::Arrive,
                &BlockGraph::search_context(),
            )
            .expect("reverse path should succeed");

        let expected = vec![
            BlockPathNode {
                block: (6, 5, 4).try_into().unwrap(),
                exit_cost: EdgeCost::JumpDown,
            },
            BlockPathNode {
                block: (5, 5, 3).try_into().unwrap(),
                exit_cost: EdgeCost::JumpDown,
            },
            BlockPathNode {
                block: (4, 5, 2).try_into().unwrap(),
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
            .fill_slice(1, DummyBlockType::Stone)
            .set_block((3, 3, 2), DummyBlockType::Stone) // step up
            .set_block((2, 3, 3), DummyBlockType::Stone) // step up from the step up
            .build((0, 0))])
        .into_inner();

        let start = (2, 3, 2); // underneath the second step
        let end = (5, 3, 2); // walk around the first step to here pls

        let path = {
            let chunk = world.find_chunk_with_pos(ChunkLocation(0, 0)).unwrap();
            let graph = chunk.block_graph_for_area(WorldArea::new((0, 0))).unwrap();
            graph
                .find_block_path(
                    start.try_into().unwrap(),
                    end.try_into().unwrap(),
                    SearchGoal::Arrive,
                    &BlockGraph::search_context(),
                )
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
