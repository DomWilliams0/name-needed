use std::collections::HashMap;

use petgraph::graph::DiGraph;

use crate::area::{Area, ChunkBoundary};
use crate::coordinate::world::WorldPosition;
use crate::Chunk;

type AreaNavGraph = DiGraph<Node, Edge>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd)]
struct Node(pub Area);

#[derive(Clone, PartialEq, Eq, Debug)]
struct Edge(pub WorldPosition);

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

                for (slab_area, link_blocks) in boundary_buf.drain() {
                    // promote to world area
                    let area = slab_area.into_area(chunk.pos());

                    for block in &link_blocks {
                        let world_pos = block.to_world_pos(chunk.pos());
                        let key = (world_pos, boundary);
                        cross_chunk_links
                            .entry(key)
                            .and_modify(|a| {
                                assert_eq!(*a, area, "must be same area in corner block links")
                            })
                            .or_insert(area);
                    }
                }
            }
        }

        // now extrapolate boundary blocks to the next chunk and link them up
        for ((block, boundary), area) in &cross_chunk_links {
            let extrapolated = boundary.shift(*block);
            let key = (extrapolated, boundary.opposite());
            let from_node = *node_lookup.get(area).expect("node should exist already");

            if let Some(other_area) = cross_chunk_links.get(&key) {
                // hurrah, add an edge between these 2 blocks
                let to_node = *node_lookup
                    .get(other_area)
                    .expect("node should exist already");
                let edge = Edge(*block);
                graph.add_edge(from_node, to_node, edge);
            } else {
                // block is in a non-existent chunk, never mind
            }
        }

        Self { graph }
    }
}

#[cfg(test)]
mod tests {
    use crate::area::AreaGraph;
    use crate::block::BlockType;
    use crate::{ChunkBuilder, ChunkPosition, CHUNK_SIZE};
    use petgraph::visit::EdgeRef;
    use petgraph::Direction;

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

    // TODO half steps and jump steps

}
