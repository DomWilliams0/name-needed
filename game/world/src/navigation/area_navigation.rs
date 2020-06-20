use std::collections::HashMap;
use std::iter::once;

use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::StableGraph;
use petgraph::Directed;

use common::*;
use unit::dim::CHUNK_SIZE;
use unit::world::{BlockCoord, BlockPosition, ChunkPosition, GlobalSliceIndex, SliceBlock};

use crate::navigation::astar::astar;
use crate::navigation::path::AreaPathNode;
use crate::navigation::{AreaPath, WorldArea};
use crate::occlusion::NeighbourOffset;
use crate::EdgeCost;

type AreaNavGraph = StableGraph<AreaNavNode, AreaNavEdge, Directed, u32>;
type NodeIndex = petgraph::prelude::NodeIndex<u32>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd)]
pub(crate) struct AreaNavNode(pub WorldArea);

#[derive(Copy, Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct AreaNavEdge {
    pub direction: NeighbourOffset,
    pub cost: EdgeCost,

    /// Block in the exiting chunk
    pub exit: BlockPosition,
    pub width: BlockCoord,
}

#[cfg_attr(test, derive(Clone))]
pub struct AreaGraph {
    graph: AreaNavGraph,
    node_lookup: HashMap<WorldArea, NodeIndex>,
}

impl Default for AreaGraph {
    fn default() -> Self {
        Self {
            graph: AreaNavGraph::with_capacity(256, 256),
            node_lookup: HashMap::with_capacity(256),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AreaPathError {
    NoSuchNode(WorldArea),
    NoPath,
}

impl AreaNavEdge {
    /// Should be sorted so BlockCoords are ascending
    pub fn discover_ports_between(
        direction: NeighbourOffset,
        connecting_blocks: impl Iterator<Item = (EdgeCost, BlockCoord, GlobalSliceIndex)>,
        out: &mut Vec<Self>,
    ) {
        let mut group_id = 0;
        connecting_blocks
            .map(|(edge, coord, slice)| (edge, coord, Some(slice)))
            .chain(once((EdgeCost::Walk, 255, None))) // dummy last
            .tuple_windows()
            .map(|((a_cost, a_coord, a_z), (b_cost, b_coord, b_z))| {
                let a_z = a_z.unwrap(); // always Some

                let diff = b_coord - a_coord;
                let this_group_id = if diff == 1 && a_cost == b_cost && Some(a_z) == b_z {
                    // group
                    group_id
                } else {
                    // next doesn't belong in this group
                    group_id += 1;
                    group_id - 1
                };

                (a_cost, a_coord, a_z, this_group_id)
            })
            .group_by(|(_, _, _, group)| *group)
            .into_iter()
            .for_each(|(_, mut ports)| {
                let (cost, start, z, _) = ports.next().unwrap(); // definitely 1
                let end = ports.last().map(|(_, end, _, _)| end).unwrap_or(start);
                let width = (end - start) + 1;

                let (x, y) = direction.position_on_boundary(start);

                out.push(Self {
                    direction,
                    cost,
                    exit: (x, y, z).into(),
                    width,
                });
            });
    }

    pub fn reversed(self) -> Self {
        let cost = self.cost.opposite();
        let direction = self.direction.opposite();

        let [mut x, mut y, mut z]: [i32; 3] = self.exit.into();

        // move to other side of the chunk
        match direction {
            NeighbourOffset::North | NeighbourOffset::South => y = CHUNK_SIZE.as_i32() - 1 - y,
            _ => x = CHUNK_SIZE.as_i32() - 1 - x,
        };

        // a reversed jump up/down requires the exit point moving down or up
        z -= cost.z_offset();

        Self {
            direction,
            cost,
            exit: BlockPosition::from((x, y, z)),
            ..self
        }
    }

    pub fn exit_middle(self) -> BlockPosition {
        let width = ((self.width - 1) / 2).max(0) as i16;
        let offset = if self.direction.is_vertical() {
            (width, 0)
        } else {
            (0, width)
        };

        match SliceBlock::from(self.exit).try_add(offset) {
            None => {
                panic!(
                    "exit width is too wide: {:?} with width {:?} and offset {:?}",
                    self.exit, self.width, offset
                );
            }
            Some(slice_block) => slice_block.to_block_position(self.exit.z()),
        }
    }
}

impl AreaGraph {
    pub(crate) fn find_area_path(
        &self,
        start: WorldArea,
        goal: WorldArea,
    ) -> Result<AreaPath, AreaPathError> {
        let src_node = self.get_node(start)?;
        let dst_node = self.get_node(goal)?;

        // node lookup should be in sync with graph
        debug_assert!(self.graph.contains_node(src_node), "start: {:?}", start);
        debug_assert!(self.graph.contains_node(dst_node), "goal: {:?}", goal);

        let path = astar(
            &self.graph,
            src_node,
            |n| n == dst_node,
            |edge| edge.weight().cost.weight(), // TODO could prefer wider ports
            |n| {
                // manhattan distance * chunk size, underestimates
                let ChunkPosition(nx, ny) = &self.graph[n].0.chunk;
                let ChunkPosition(gx, gy) = goal.chunk;

                let dx = (nx - gx).abs() * CHUNK_SIZE.as_i32();
                let dy = (ny - gy).abs() * CHUNK_SIZE.as_i32();
                (dx + dy) as f32
            },
        )
        .ok_or(AreaPathError::NoPath)?;

        let mut out_path = Vec::with_capacity(path.len() + 1);

        // first is a special case and unconditional
        out_path.push(AreaPathNode::new_start(start));

        let area_nodes = path
            .into_iter()
            .map(|(node, edge)| (self.graph[node].0, self.graph[edge]));
        for (area, edge) in area_nodes {
            out_path.push(AreaPathNode::new(area, edge));
        }

        Ok(AreaPath(out_path))
    }

    pub(crate) fn path_exists(&self, start: WorldArea, goal: WorldArea) -> bool {
        // TODO dont allocate and throw away path
        self.find_area_path(start, goal).is_ok()
    }

    pub(crate) fn add_edge(&mut self, from: WorldArea, to: WorldArea, edge: AreaNavEdge) {
        info!("edge {:?} <-> {:?} | {:?}", from, to, edge);

        let (a, b) = (self.add_node(from), self.add_node(to));
        self.graph.add_edge(a, b, edge);
        self.graph.add_edge(b, a, edge.reversed());
    }

    pub(crate) fn add_node(&mut self, area: WorldArea) -> NodeIndex {
        match self.node_lookup.get(&area) {
            Some(n) => *n,
            None => {
                debug_assert!(
                    self.graph
                        .node_indices()
                        .find(|n| self.graph.node_weight(*n).unwrap().0 == area)
                        .is_none(),
                    "node is not in both lookup and graph"
                );
                let n = self.graph.add_node(AreaNavNode(area));
                self.node_lookup.insert(area, n);
                n
            }
        }
    }

    pub(crate) fn remove_node(&mut self, area: &WorldArea) {
        if let Some(node) = self.node_lookup.remove(area) {
            // invalidate edges first
            let edges: Vec<_> = self.graph.edges(node).map(|e| e.id()).collect();
            for edge in edges {
                self.graph.remove_edge(edge);
            }

            // invalidate node
            let old = self.graph.remove_node(node);
            debug_assert!(old.is_some(), "node was not in both lookup and graph")
        }
    }

    fn get_node(&self, area: WorldArea) -> Result<NodeIndex, AreaPathError> {
        self.node_lookup
            .get(&area)
            .copied()
            .ok_or_else(|| AreaPathError::NoSuchNode(area))
    }

    #[cfg(test)]
    fn get_edges(&self, from: WorldArea, to: WorldArea) -> Vec<AreaNavEdge> {
        use petgraph::prelude::Direction;

        match (self.get_node(from), self.get_node(to)) {
            (Ok(from), Ok(to)) => self
                .graph
                .edges_directed(from, Direction::Outgoing)
                .filter(|e| e.target() == to)
                .map(|e| *e.weight())
                .collect(),
            _ => Vec::new(),
        }
    }

    #[cfg(test)]
    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    #[cfg(test)]
    fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;

    use common::*;
    use unit::dim::CHUNK_SIZE;
    use unit::world::{BlockPosition, ChunkPosition, GlobalSliceIndex, SlabIndex, SLAB_SIZE};

    use crate::block::BlockType;
    use crate::chunk::ChunkBuilder;
    use crate::navigation::path::AreaPathNode;
    use crate::navigation::{AreaGraph, AreaNavEdge, AreaPathError, SlabAreaIndex, WorldArea};
    use crate::occlusion::NeighbourOffset;
    use crate::world::helpers::world_from_chunks;
    use crate::{ChunkDescriptor, EdgeCost};

    fn make_graph(chunks: Vec<ChunkDescriptor>) -> AreaGraph {
        // gross but allows for neater tests
        let world = world_from_chunks(chunks).into_inner();
        (*world.area_graph()).clone()
    }

    fn get_edge(graph: &AreaGraph, from: WorldArea, to: WorldArea) -> Option<AreaNavEdge> {
        let mut edges = graph.get_edges(from, to).into_iter();
        let edge = edges.next();
        assert!(
            edges.next().is_none(),
            "1 edge expected but found {}",
            edges.len() + 1
        );
        edge
    }

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

        let graph = make_graph(chunks);

        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 2);
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

        let graph = make_graph(chunks);
        assert_eq!(graph.graph.node_count(), 3);
        assert_eq!(graph.graph.edge_count(), 2 * 2);

        // get area at 0, 0
        let zerozero = WorldArea::new((0, 0));

        let _ = get_edge(&graph, zerozero, WorldArea::new((1, 0)))
            .expect("edge to (1, 0) should exist");

        let _ = get_edge(&graph, zerozero, WorldArea::new((0, -1)))
            .expect("edge to (0, -1) should exist");
    }

    #[test]
    fn cross_chunk_area_linkage() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        // step up from chunk 0 (0,5,N-1) to chunk 1 (-1, 5, N) slab 1
        let graph = make_graph(vec![
            ChunkBuilder::new()
                .set_block((0, 5, -1), BlockType::Grass)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((CHUNK_SIZE.as_i32() - 1, 5, 0), BlockType::Stone)
                .build((-1, 0)),
        ]);

        assert_eq!(graph.node_count(), 2); // only 1 area in each chunk
        assert_eq!(graph.edge_count(), 2); // 1 edge each way

        let a = WorldArea::new((0, 0));
        let b = WorldArea::new((-1, 0));

        let _ = get_edge(&graph, a, b).expect("edge should exist");
        let _ = get_edge(&graph, b, a).expect("node should exist both ways");
    }

    #[test]
    fn cross_chunk_area_linkage_cross_slab() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        // -1, 5, 16 -> 0, 5, 15
        let graph = make_graph(vec![
            ChunkBuilder::new()
                .set_block((0, 5, SLAB_SIZE.as_i32() - 1), BlockType::Grass)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block(
                    (CHUNK_SIZE.as_i32() - 1, 5, SLAB_SIZE.as_i32()),
                    BlockType::Stone,
                )
                .build((-1, 0)),
        ]);

        assert_eq!(graph.node_count(), 2); // 1 area in (0,0) and 1 in (-1,0)
        assert_eq!(graph.edge_count(), 1 * 2); // 1 each way

        let a = WorldArea {
            chunk: ChunkPosition(0, 0),
            slab: 1.into(),
            area: SlabAreaIndex::FIRST,
        };
        let b = WorldArea {
            chunk: ChunkPosition(-1, 0),
            slab: 1.into(),
            area: SlabAreaIndex::FIRST,
        };

        let _ = get_edge(&graph, a, b).expect("edge should exist");
        let _ = get_edge(&graph, b, a).expect("node should exist both ways");
    }
    #[test]
    fn empty_slab_no_areas() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let graph = make_graph(vec![ChunkBuilder::new()
            // 1 block in second slab
            .set_block((2, 2, SLAB_SIZE.as_i32()), BlockType::Stone)
            .build((0, 0))]);

        assert_eq!(graph.node_count(), 1); // just one area in slab idx 1
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

        let graph = make_graph(chunks);

        // 0, 0 should have edges along each side
        assert_eq!(graph.node_count(), 5);
        assert_eq!(
            graph
                .graph
                .edge_indices()
                .map(|e| (graph.graph[e], graph.graph.edge_endpoints(e).unwrap()))
                .filter(|(_, (a, b))| graph.graph[*a].0.chunk == ChunkPosition(0, 0)
                    || graph.graph[*b].0.chunk == ChunkPosition(0, 0))
                .count(),
            4 * 2
        );
    }

    #[test]
    fn pure_port_discovery() {
        // pure = isolated test
        let link_blocks = vec![
            // one group
            (EdgeCost::Walk, 0, GlobalSliceIndex::new(0)),
            (EdgeCost::Walk, 1, GlobalSliceIndex::new(0)),
            (EdgeCost::Walk, 2, GlobalSliceIndex::new(0)),
            // another group
            (EdgeCost::Walk, 4, GlobalSliceIndex::new(0)),
            (EdgeCost::Walk, 5, GlobalSliceIndex::new(0)),
            (EdgeCost::Walk, 6, GlobalSliceIndex::new(0)),
            // another group
            (EdgeCost::JumpUp, 7, GlobalSliceIndex::new(0)),
            (EdgeCost::JumpUp, 8, GlobalSliceIndex::new(0)),
            // all alone groups
            (EdgeCost::JumpUp, 10, GlobalSliceIndex::new(0)),
            (EdgeCost::JumpUp, 11, GlobalSliceIndex::new(5)), // different z
            (EdgeCost::JumpDown, 12, GlobalSliceIndex::new(5)), // different cost
        ];

        let direction = NeighbourOffset::West;
        let mut ports = vec![];
        AreaNavEdge::discover_ports_between(direction, link_blocks.into_iter(), &mut ports);

        let expected = vec![
            AreaNavEdge {
                cost: EdgeCost::Walk,
                width: 3,
                exit: BlockPosition::new(0, 0, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::Walk,
                width: 3,
                exit: BlockPosition::new(0, 4, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpUp,
                width: 2,
                exit: BlockPosition::new(0, 7, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpUp,
                width: 1,
                exit: BlockPosition::new(0, 10, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpUp,
                width: 1,
                exit: BlockPosition::new(0, 11, GlobalSliceIndex::new(5)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpDown,
                width: 1,
                exit: BlockPosition::new(0, 12, GlobalSliceIndex::new(5)),
                direction,
            },
        ];

        assert_eq!(ports, expected);
    }

    #[test]
    fn world_port_discovery() {
        let graph = make_graph(vec![
            ChunkBuilder::new()
                .fill_slice(3, BlockType::Stone)
                .build((-4, -4)),
            ChunkBuilder::new()
                // 3 wide port
                .set_block((0, 5, 3), BlockType::Grass)
                .set_block((0, 6, 3), BlockType::Grass)
                .set_block((0, 7, 3), BlockType::Grass)
                // little bridge to have 1 connected area
                .set_block((1, 7, 3), BlockType::Stone)
                .set_block((1, 8, 3), BlockType::Stone)
                .set_block((1, 9, 4), BlockType::Stone)
                .set_block((1, 10, 4), BlockType::Stone)
                // another disconnected 1 wide port
                .set_block((0, 10, 4), BlockType::Grass)
                .build((-3, -4)),
            ChunkBuilder::new().build((0, 0)),
        ]);

        let mut edges = graph.get_edges(WorldArea::new((-4, -4)), WorldArea::new((-3, -4)));

        let mut expected = vec![
            AreaNavEdge {
                direction: NeighbourOffset::East,
                cost: EdgeCost::Walk,
                exit: (15, 5, 4).into(),
                width: 3,
            },
            AreaNavEdge {
                direction: NeighbourOffset::East,
                cost: EdgeCost::JumpUp,
                exit: (15, 10, 4).into(),
                width: 1,
            },
        ];

        edges.sort_by_key(|e| e.exit.y());
        expected.sort_by_key(|e| e.exit.y());

        assert_eq!(edges, expected);
    }

    #[test]
    fn area_path_ring_all_directions() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let graph = make_graph(crate::presets::ring());

        // world is based at z=300
        const SLAB: SlabIndex = SlabIndex(300 / SLAB_SIZE.as_i32());

        {
            // from top left to top right
            // path crosses south,east,north boundaries because theres no east/west bridge between top 2
            let path = graph
                .find_area_path(
                    WorldArea::new_with_slab((-1, 1), SLAB),
                    WorldArea::new_with_slab((0, 1), SLAB),
                )
                .expect("path should succeed");

            let expected = vec![
                AreaPathNode::new_start(WorldArea::new_with_slab((-1, 1), SLAB)),
                // south
                AreaPathNode::new(
                    WorldArea::new_with_slab((-1, 0), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::South,
                        cost: EdgeCost::JumpUp,
                        exit: (3, 0, 301).into(),
                        width: 1,
                    },
                ),
                // east
                AreaPathNode::new(
                    WorldArea::new_with_slab((0, 0), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::East,
                        cost: EdgeCost::JumpDown,
                        exit: (CHUNK_SIZE.as_block_coord() - 1, 3, 302).into(),
                        width: 1,
                    },
                ),
                // north
                AreaPathNode::new(
                    WorldArea::new_with_slab((0, 1), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::North,
                        cost: EdgeCost::JumpUp,
                        exit: (3, CHUNK_SIZE.as_block_coord() - 1, 301).into(),
                        width: 1,
                    },
                ),
            ];

            assert_eq!(path.0, expected);
        }

        {
            // from top right to top left
            // path crosses south,west,north boundaries this time
            let path = graph
                .find_area_path(
                    WorldArea::new_with_slab((0, 1), SLAB),
                    WorldArea::new_with_slab((-1, 1), SLAB),
                )
                .expect("path should succeed");

            let expected = vec![
                AreaPathNode::new_start(WorldArea::new_with_slab((0, 1), SLAB)),
                // south
                AreaPathNode::new(
                    WorldArea::new_with_slab((0, 0), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::South,
                        cost: EdgeCost::JumpDown,
                        exit: (3, 0, 302).into(),
                        width: 1,
                    },
                ),
                // west
                AreaPathNode::new(
                    WorldArea::new_with_slab((-1, 0), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::West,
                        cost: EdgeCost::JumpUp,
                        exit: (0, 3, 301).into(),
                        width: 1,
                    },
                ),
                // north
                AreaPathNode::new(
                    WorldArea::new_with_slab((-1, 1), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::North,
                        cost: EdgeCost::JumpDown,
                        exit: (3, CHUNK_SIZE.as_block_coord() - 1, 302).into(),
                        width: 1,
                    },
                ),
            ];

            assert_eq!(path.0, expected);
        }
    }

    #[test]
    fn area_path_across_three_chunks() {
        let graph = make_graph(vec![
            ChunkBuilder::new()
                // 2 wide port going east
                .set_block((14, 2, 1), BlockType::Stone)
                .set_block((14, 3, 1), BlockType::Stone)
                .set_block((15, 2, 2), BlockType::Stone)
                .set_block((15, 3, 2), BlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(3, BlockType::Grass)
                .build((0, 0)),
            ChunkBuilder::new()
                // 1 wide port still going east
                .set_block((0, 5, 2), BlockType::Stone)
                .set_block((1, 5, 1), BlockType::Stone)
                .build((1, 0)),
        ]);
        let path = graph
            .find_area_path(WorldArea::new((-1, 0)), WorldArea::new((1, 0)))
            .expect("path should succeed");

        let expected = vec![
            AreaPathNode::new_start(WorldArea::new((-1, 0))),
            AreaPathNode::new(
                WorldArea::new((0, 0)),
                AreaNavEdge {
                    direction: NeighbourOffset::East,
                    cost: EdgeCost::JumpUp,
                    exit: (15, 2, 3).into(),
                    width: 2,
                },
            ),
            AreaPathNode::new(
                WorldArea::new((1, 0)),
                AreaNavEdge {
                    direction: NeighbourOffset::East,
                    cost: EdgeCost::JumpDown,
                    exit: (15, 5, 4).into(),
                    width: 1,
                },
            ),
        ];

        assert_eq!(path.0, expected);
    }

    #[test]
    fn area_path_across_two_chunks() {
        // also the blocks are ridiculously high and not in slab 0
        const SLAB: SlabIndex = SlabIndex(201 / SLAB_SIZE.as_i32());

        let w = world_from_chunks(vec![
            ChunkBuilder::new()
                .set_block((14, 2, 201), BlockType::Stone)
                .set_block((15, 2, 201), BlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(201, BlockType::Grass)
                .build((0, 0)),
        ])
        .into_inner();

        let path = w
            .find_area_path(
                (-2, 2, 202),  // chunk -1, 0
                (10, 10, 202), // chunk 0, 0
            )
            .expect("path should succeed");

        let expected = vec![
            AreaPathNode::new_start(WorldArea::new_with_slab((-1, 0), SLAB)),
            AreaPathNode::new(
                WorldArea::new_with_slab((0, 0), SLAB),
                AreaNavEdge {
                    direction: NeighbourOffset::East,
                    cost: EdgeCost::Walk,
                    exit: (15, 2, 202).into(),
                    width: 1,
                },
            ),
        ];

        assert_eq!(path.0, expected);
    }

    #[test]
    fn area_path_within_single_chunk() {
        // also the blocks are ridiculously high and not in slab 0
        const SLAB: SlabIndex = SlabIndex(202 / SLAB_SIZE.as_i32());

        let w = world_from_chunks(vec![ChunkBuilder::new()
            .fill_slice(201, BlockType::Grass)
            .build((0, 0))])
        .into_inner();

        let path = w
            .find_area_path(
                (2, 2, 202), // chunk 0, 0
                (8, 3, 202), // also chunk 0, 0
            )
            .expect("path should succeed");

        assert_eq!(
            path.0,
            vec![AreaPathNode::new_start(WorldArea::new_with_slab(
                (0, 0),
                SLAB
            ))]
        );
    }

    #[test]
    fn area_path_bad() {
        let graph = make_graph(vec![ChunkBuilder::new()
            .fill_slice(1, BlockType::Grass)
            .build((0, 0))]);

        let err = graph.find_area_path(WorldArea::new((0, 0)), WorldArea::new((100, 20)));

        assert_matches!(err, Err(AreaPathError::NoSuchNode(_)));
    }

    #[test]
    fn area_edge_reverse() {
        let edge = AreaNavEdge {
            direction: NeighbourOffset::South,
            cost: EdgeCost::JumpUp,
            exit: (5, 0, 5).into(),
            width: 2,
        };

        let reversed = AreaNavEdge {
            direction: NeighbourOffset::North,
            cost: EdgeCost::JumpDown,
            exit: BlockPosition::new(5, CHUNK_SIZE.as_block_coord() - 1, GlobalSliceIndex::new(6)),
            width: 2,
        };

        assert_eq!(edge.reversed(), reversed);
        assert_eq!(reversed.reversed(), edge);
    }

    #[test]
    fn port_exit_middle() {
        assert_eq!(
            AreaNavEdge {
                direction: NeighbourOffset::South,
                cost: EdgeCost::Walk,
                exit: (4, 4, 4).into(),
                width: 1
            }
            .exit_middle(),
            (4, 4, 4).into()
        );

        assert_eq!(
            AreaNavEdge {
                direction: NeighbourOffset::South,
                cost: EdgeCost::Walk,
                exit: (4, 4, 4).into(),
                width: 5
            }
            .exit_middle(),
            (6, 4, 4).into()
        );

        assert_eq!(
            AreaNavEdge {
                direction: NeighbourOffset::West,
                cost: EdgeCost::Walk,
                exit: (0, 0, 1).into(),
                width: 5
            }
            .exit_middle(),
            (0, 2, 1).into()
        );
    }
}
