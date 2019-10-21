use std::collections::HashMap;
use std::iter::once;

use cgmath::MetricSpace;
use cgmath::Vector3;
use itertools::Itertools;
use petgraph::algo::astar;
use petgraph::graph::DiGraph;

use crate::area::path::{AreaPath, AreaPathNode};
use crate::area::{ChunkBoundary, EdgeCost, WorldArea};
use crate::coordinate::world::WorldPosition;
use crate::Chunk;
use petgraph::prelude::NodeIndex;

type AreaNavGraph = DiGraph<Node, Edge>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd)]
struct Node(pub WorldArea);

#[derive(Clone, PartialEq, Eq, Debug)]
struct Edge {
    /// in another area
    pub from: WorldPosition,

    /// in this area
    pub to: WorldPosition,

    pub cost: EdgeCost,
}

pub struct AreaGraph {
    graph: AreaNavGraph,
}

impl AreaGraph {
    pub fn from_chunks(chunks: &[Chunk]) -> Self {
        let mut graph = AreaNavGraph::new();
        let mut boundary_buf = HashMap::new();
        let mut cross_chunk_links = HashMap::new();
        let mut node_lookup = HashMap::new();

        // add area nodes
        for chunk in chunks {
            for area in chunk.areas() {
                let idx = graph.add_node(Node(*area));
                node_lookup.insert(*area, idx);
            }

            // TODO add chunk-internal links when they actually exist
        }

        // read boundary blocks and store them with their corresponding area
        for chunk in chunks {
            for boundary in ChunkBoundary::boundaries() {
                // get boundary areas and their link blocks
                chunk.areas_for_boundary(boundary, &mut boundary_buf);

                for (chunk_area, link_blocks) in boundary_buf.drain() {
                    // promote to world area
                    let world_area = chunk_area.into_world_area(chunk.pos());

                    for block in &link_blocks {
                        let world_pos = block.to_world_pos(chunk.pos());
                        let key = (world_pos, boundary);
                        let value = (world_area, chunk.get_block(*block).unwrap().block_height());
                        cross_chunk_links
                            .entry(key)
                            .and_modify(|(a, _)| {
                                assert_eq!(
                                    *a, world_area,
                                    "must be same area in corner block links"
                                )
                            })
                            .or_insert(value);
                    }
                }
            }
        }

        // now extrapolate boundary blocks to the next chunk and link them up
        for ((block, boundary), (area, height)) in &cross_chunk_links {
            let (extrapolated, other_area, other_height) = {
                // try the next block over
                let shifted = boundary.shift(*block);
                let opposite = boundary.opposite();

                let mut extrapolated = shifted;
                let result = cross_chunk_links
                    .get(&(extrapolated, opposite))
                    .or_else(|| {
                        // try the one above
                        extrapolated.2 = shifted.2 + 1;
                        cross_chunk_links.get(&(extrapolated, opposite))
                    })
                    .or_else(|| {
                        // try the one below
                        extrapolated.2 = shifted.2 - 1;
                        cross_chunk_links.get(&(extrapolated, opposite))
                    });

                if let Some((area, height)) = result {
                    // nice, we found an area to link with
                    (extrapolated, *area, *height)
                } else {
                    // block is in a non-existent chunk, never mind
                    continue;
                }
            };

            // add an edge between these areas via these 2 blocks
            let from_node = *node_lookup.get(&area).unwrap();
            let to_node = *node_lookup.get(&other_area).unwrap();
            let edge = {
                let cost = {
                    let z_diff = extrapolated.2 - block.2;
                    EdgeCost::from_height_diff(*height, other_height, z_diff)
                        .expect("boundary blocks should be adjacent")
                };
                Edge {
                    from: *block,
                    to: extrapolated,
                    cost,
                }
            };

            graph.add_edge(from_node, to_node, edge);
        }

        Self { graph }
    }

    fn get_node_index(&self, area: WorldArea) -> Option<NodeIndex> {
        self.graph
            .node_indices()
            .find(|n| self.graph.node_weight(*n).unwrap().0 == area)
    }

    fn get_node(&self, index: NodeIndex) -> Option<WorldArea> {
        self.graph.node_weight(index).map(|n| n.0)
    }

    fn get_edge(&self, from: NodeIndex, to: NodeIndex) -> Option<&Edge> {
        self.graph
            .find_edge(from, to)
            .and_then(|e| self.graph.edge_weight(e))
    }

    pub(crate) fn find_area_path(&self, start: WorldArea, goal: WorldArea) -> Option<AreaPath> {
        let (from_node, to_node) = match (self.get_node_index(start), self.get_node_index(goal)) {
            (Some(from), Some(to)) => (from, to),
            _ => return None,
        };

        let to_vec = Vector3::from(goal);
        match astar(
            &self.graph,
            from_node,
            |n| n == to_node,
            |e| e.weight().cost.weight(),
            |n| {
                let here = self.graph.node_weight(n).unwrap().0;
                let here_vec = Vector3::from(here);
                to_vec.distance2(here_vec) as i32
            },
        ) {
            Some((_cost, nodes)) => {
                // wrap each in Option with a None prepended and appended
                let nodes = once(None).chain(nodes.iter().map(Some)).chain(once(None));

                let nodes = nodes
                    .tuple_windows()
                    .filter_map(|chunk| {
                        match chunk {
                            (None, Some(&b), Some(&c)) => {
                                // `b` is first - no entry
                                let b2c = self.get_edge(b, c).expect("edge should exist");

                                Some(AreaPathNode {
                                    area: self.get_node(b).expect("node should exist"),
                                    entry: None,
                                    exit: Some((b2c.from, b2c.cost)),
                                })
                            }

                            (Some(&a), Some(&b), Some(&c)) => {
                                // `b` has an entry and exit edge
                                let a2b = self.get_edge(a, b).expect("edge should exist");
                                let b2c = self.get_edge(b, c).expect("edge should exist");

                                Some(AreaPathNode {
                                    area: self.get_node(b).expect("node should exist"),
                                    entry: Some((a2b.to, a2b.cost)),
                                    exit: Some((b2c.from, b2c.cost)),
                                })
                            }

                            (Some(&a), Some(&b), None) => {
                                // `b` is last - no exit
                                let a2b = self.get_edge(a, b).expect("edge should exist");

                                Some(AreaPathNode {
                                    area: self.get_node(b).expect("node should exist"),
                                    entry: Some((a2b.to, a2b.cost)),
                                    exit: None,
                                })
                            }
                            (None, Some(&area), None) => {
                                // there is only a single area
                                Some(AreaPathNode {
                                    area: self.get_node(area).expect("node should exist"),
                                    entry: None,
                                    exit: None,
                                })
                            }

                            bad => unreachable!("unexpected {:?}", bad),
                        }
                    })
                    .collect_vec();

                Some(AreaPath(nodes))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ordered_float::OrderedFloat;
    use petgraph::visit::EdgeRef;
    use petgraph::Direction;

    use crate::area::AreaGraph;
    use crate::area::EdgeCost;
    use crate::block::{BlockHeight, BlockType};
    use crate::{ChunkBuilder, ChunkPosition, CHUNK_SIZE};

    #[test]
    fn one_block_one_side_flat() {
        let chunks = vec![
            ChunkBuilder::new()
                .set_block((15, 5, 0), BlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 5, 0), BlockType::Grass)
                .build((1, 0)),
        ];

        let graph = AreaGraph::from_chunks(&chunks);
        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 2); // 1 each way
    }

    #[test]
    fn one_block_two_sides_flat() {
        let chunks = vec![
            ChunkBuilder::new()
                .set_block((15, 0, 0), BlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 0, 0), BlockType::Grass)
                .build((1, 0)),
            ChunkBuilder::new()
                .set_block((15, 15, 0), BlockType::Grass)
                .build((0, -1)),
        ];

        let graph = AreaGraph::from_chunks(&chunks);
        assert_eq!(graph.graph.node_count(), 3);

        // get area at 0, 0 in a disgusting manner
        let zerozero = graph
            .graph
            .node_indices()
            .find(|n| graph.graph.node_weight(*n).unwrap().0.chunk == (0, 0).into())
            .unwrap();

        assert_eq!(
            graph
                .graph
                .edges_directed(zerozero, Direction::Outgoing)
                .count(),
            2
        ); // 1 out to each other chunk

        assert_eq!(
            graph
                .graph
                .edges_directed(zerozero, Direction::Incoming)
                .count(),
            2
        ); // 1 in from each other chunk too
    }

    #[test]
    fn full_slices_flat() {
        let chunks = vec![
            ChunkBuilder::new()
                .fill_slice(0, BlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .fill_slice(0, BlockType::Stone)
                .build((1, 0)),
            ChunkBuilder::new()
                .fill_slice(0, BlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(0, BlockType::Stone)
                .build((0, 1)),
            ChunkBuilder::new()
                .fill_slice(0, BlockType::Stone)
                .build((0, -1)),
        ];

        let graph = AreaGraph::from_chunks(&chunks);
        assert_eq!(graph.graph.node_count(), 5);

        // 0, 0 should have edges along each side
        assert_eq!(
            graph
                .graph
                .edge_references()
                .filter(|e| {
                    let to = graph.graph.node_weight(e.target()).unwrap().0;
                    to.chunk == ChunkPosition(0, 0)
                })
                .count(),
            CHUNK_SIZE.as_usize() * 4
        );
    }

    #[test]
    fn half_step() {
        // the edge between 2 areas should take into the account if its a jump/half step
        let graph = AreaGraph::from_chunks(&vec![
            ChunkBuilder::new()
                .set_block((15, 5, 0), BlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 5, 1), (BlockType::Grass, BlockHeight::Half))
                .build((1, 0)),
        ]);
        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 2); // 1 each way

        // chunk 0, 0
        let node_a = graph
            .graph
            .node_indices()
            .find(|n| {
                let area = graph.graph.node_weight(*n).unwrap().0;
                area.chunk.0 == 0
            })
            .unwrap();

        // chunk 1, 0
        let node_b = graph.graph.node_indices().find(|n| *n != node_a).unwrap();

        let edge_up = graph.get_edge(node_a, node_b).unwrap();
        let edge_down = graph.get_edge(node_b, node_a).unwrap();

        let half_height = BlockHeight::Half.height();
        assert_eq!(edge_up.cost, EdgeCost::Step(OrderedFloat(half_height)));
        assert_eq!(edge_down.cost, EdgeCost::Step(OrderedFloat(-half_height)));
    }

    #[test]
    fn jump() {
        // the edge between 2 areas should take into the account if its a jump/half step
        let graph = AreaGraph::from_chunks(&vec![
            ChunkBuilder::new()
                .set_block((15, 5, 0), BlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 5, 1), BlockType::Grass)
                .build((1, 0)),
        ]);
        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 2); // 1 each way

        // chunk 0, 0
        let node_a = graph
            .graph
            .node_indices()
            .find(|n| {
                let area = graph.graph.node_weight(*n).unwrap().0;
                area.chunk.0 == 0
            })
            .unwrap();

        // chunk 1, 0
        let node_b = graph.graph.node_indices().find(|n| *n != node_a).unwrap();

        let edge_up = graph.get_edge(node_a, node_b).unwrap();
        let edge_down = graph.get_edge(node_b, node_a).unwrap();

        let _half_height = BlockHeight::Half.height();
        assert_eq!(edge_up.cost, EdgeCost::JumpUp);
        assert_eq!(edge_down.cost, EdgeCost::JumpDown);
    }

}
