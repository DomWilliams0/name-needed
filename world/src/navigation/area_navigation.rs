use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::iter::once;

use petgraph::graph::EdgeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::Visitable;
use petgraph::Directed;

use misc::*;
use unit::world::CHUNK_SIZE;
use unit::world::{BlockCoord, BlockPosition, ChunkLocation, GlobalSliceIndex, SliceBlock};

use crate::navigation::path::AreaPathNode;
use crate::navigation::search::{astar, SearchContext};
use crate::navigation::{AreaPath, WorldArea};
use crate::neighbour::NeighbourOffset;
use crate::EdgeCost;

type AreaNavGraph = StableGraph<AreaNavNode, AreaNavEdge, Directed, u32>;
type NodeIndex = petgraph::prelude::NodeIndex<u32>;

pub type AreaGraphSearchContext =
    SearchContext<NodeIndex, EdgeIndex, f32, <AreaNavGraph as Visitable>::Map>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd)]
pub struct AreaNavNode(pub WorldArea);

#[derive(Copy, Clone)]
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
    // TODO use graphmap to just use areas as nodes? but we need parallel edges
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

#[derive(Debug, Clone, Error)]
pub enum AreaPathError {
    #[error("No such area {0:?}")]
    NoSuchNode(WorldArea),

    #[error("No path found")]
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

        let exit = {
            let (mut x, mut y, mut z) = self.exit.xyz();

            // move to other side of the chunk
            match direction {
                NeighbourOffset::North | NeighbourOffset::South => {
                    y = CHUNK_SIZE.as_block_coord() - 1 - y
                }
                _ => x = CHUNK_SIZE.as_block_coord() - 1 - x,
            };

            // a reversed jump up/down requires the exit point moving down or up
            z -= cost.z_offset();

            BlockPosition::new_unchecked(x, y, z)
        };

        Self {
            direction,
            cost,
            exit,
            ..self
        }
    }

    fn iter_exit_blocks(&self) -> impl Iterator<Item = SliceBlock> + '_ {
        let start = SliceBlock::from(self.exit);
        (0..self.width as i16).map(move |i| {
            let offset = if self.direction.is_vertical() {
                (i, 0)
            } else {
                (0, i)
            };

            start.try_add(offset).unwrap_or_else(|| {
                unreachable!(
                    "exit width is too wide: {:?} with width {:?} at offset {}",
                    self.exit, self.width, i
                )
            })
        })
    }

    /// Finds the block along the full width of the port that is closest to the given source block.
    pub fn exit_closest(self, source: BlockPosition) -> BlockPosition {
        let (src_x, src_y) = (source.x() as i16, source.y() as i16);
        self.iter_exit_blocks()
            .min_by_key(|candidate| {
                let (x, y) = candidate.xy();
                let dx = (x as i16 - src_x).abs();
                let dy = (y as i16 - src_y).abs();
                dx + dy
            })
            .expect("exit cannot be zero width")
            .to_block_position(self.exit.z())
    }

    pub fn contains(&self, block: BlockPosition) -> bool {
        block.z() == self.exit.z()
            && self
                .iter_exit_blocks()
                .any(|b| b.xy() == (block.x(), block.y()))
    }
}

impl AreaGraph {
    pub fn search_context() -> AreaGraphSearchContext {
        AreaGraphSearchContext::new::<AreaNavGraph>()
    }

    pub(crate) fn find_area_path(
        &self,
        start: WorldArea,
        goal: WorldArea,
        context: &AreaGraphSearchContext,
    ) -> Result<AreaPath, AreaPathError> {
        let src_node = self.get_node(start)?;
        let dst_node = self.get_node(goal)?;

        // node lookup should be in sync with graph
        debug_assert!(self.graph.contains_node(src_node), "start: {:?}", start);
        debug_assert!(self.graph.contains_node(dst_node), "goal: {:?}", goal);

        astar(
            &self.graph,
            src_node,
            |n| n == dst_node,
            |edge| edge.weight().cost.weight(), // TODO could prefer wider ports
            |n| {
                // manhattan distance * chunk size, underestimates
                let ChunkLocation(nx, ny) = &self.graph[n].0.chunk;
                let ChunkLocation(gx, gy) = goal.chunk;

                let dx = (nx - gx).abs() * CHUNK_SIZE.as_i32();
                let dy = (ny - gy).abs() * CHUNK_SIZE.as_i32();
                (dx + dy) as f32
            },
            context,
        );

        let path = &*context.result();
        if path.is_empty() && src_node != dst_node {
            return Err(AreaPathError::NoPath);
        }

        let mut out_path = Vec::with_capacity(path.len() + 1);

        // first is a special case and unconditional
        out_path.push(AreaPathNode::new_start(start));

        let area_nodes = path
            .iter()
            .map(|&(node, edge)| (self.graph[node].0, self.graph[edge]));
        for (area, edge) in area_nodes {
            out_path.push(AreaPathNode::new(area, edge));
        }

        Ok(AreaPath(out_path))
    }

    pub(crate) fn get_adjacent_area_edge(
        &self,
        from: WorldArea,
        to: WorldArea,
    ) -> Option<&AreaNavEdge> {
        let src_node = self.get_node(from).ok()?;
        let dst_node = self.get_node(to).ok()?;

        // node lookup should be in sync with graph
        debug_assert!(self.graph.contains_node(src_node), "start: {:?}", from);
        debug_assert!(self.graph.contains_node(dst_node), "goal: {:?}", to);

        self.graph
            .find_edge(src_node, dst_node)
            .map(|e| self.graph.edge_weight(e).expect("bad edge"))
    }

    pub(crate) fn path_exists(
        &self,
        start: WorldArea,
        goal: WorldArea,
        context: &AreaGraphSearchContext,
    ) -> bool {
        // TODO avoid calculating path just to throw it away
        self.find_area_path(start, goal, context).is_ok()
    }

    pub(crate) fn add_edge(&mut self, from: WorldArea, to: WorldArea, edge: AreaNavEdge) {
        debug!("adding 2-way edge"; "source" => ?from, "dest" => ?to, "edge" => ?edge);

        let (a, b) = (self.add_node(from), self.add_node(to));
        self.graph.add_edge(a, b, edge);
        self.graph.add_edge(b, a, edge.reversed());
    }

    pub(crate) fn add_node(&mut self, area: WorldArea) -> NodeIndex {
        match self.node_lookup.get(&area) {
            Some(n) => *n,
            None => {
                debug_assert!(
                    !self
                        .graph
                        .node_indices()
                        .any(|n| self.graph.node_weight(n).unwrap().0 == area),
                    "node is not in both lookup and graph"
                );
                let n = self.graph.add_node(AreaNavNode(area));
                self.node_lookup.insert(area, n);
                n
            }
        }
    }

    // pub(crate) fn remove_node(&mut self, area: &WorldArea) {
    //     if let Some(node) = self.node_lookup.remove(area) {
    //         // invalidate node, which removes all its edges too
    //         let old = self.graph.remove_node(node);
    //         debug_assert!(old.is_some(), "node was not in both lookup and graph")
    //     }
    // }

    /// Removes all where f(area) == false.
    /// Returns number removed
    pub(crate) fn retain(&mut self, mut f: impl FnMut(&WorldArea) -> bool) -> usize {
        let prev_n = (self.node_lookup.len(), self.graph.node_count());
        debug_assert_eq!(prev_n.0, prev_n.1);

        self.node_lookup.retain(|n, _| f(n));
        self.graph.retain_nodes(|graph, idx| {
            let node = graph.node_weight(idx).unwrap();
            f(&node.0)
        });

        let new_n = (self.node_lookup.len(), self.graph.node_count());
        debug_assert_eq!(new_n.0, new_n.1);
        prev_n.0 - new_n.0
    }

    fn get_node(&self, area: WorldArea) -> Result<NodeIndex, AreaPathError> {
        self.node_lookup
            .get(&area)
            .copied()
            .ok_or(AreaPathError::NoSuchNode(area))
    }

    #[cfg(test)]
    fn get_edges(&self, from: WorldArea, to: WorldArea) -> Vec<AreaNavEdge> {
        use petgraph::prelude::*;

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

impl Debug for AreaNavEdge {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AreaNavEdge(direction={:?}, {:?}, exit={}, width={})",
            self.direction, self.cost, self.exit, self.width
        )
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use unit::world::CHUNK_SIZE;
    use unit::world::{BlockPosition, ChunkLocation, GlobalSliceIndex, SlabIndex, SLAB_SIZE};

    use crate::chunk::ChunkBuilder;
    use crate::helpers::{DummyBlockType, DummyWorldContext};
    use crate::navigation::path::AreaPathNode;
    use crate::navigation::{AreaGraph, AreaNavEdge, AreaPathError, SlabAreaIndex, WorldArea};
    use crate::neighbour::NeighbourOffset;
    use crate::world::helpers::world_from_chunks_blocking;
    use crate::{ChunkDescriptor, EdgeCost};

    fn make_graph(chunks: Vec<ChunkDescriptor<DummyWorldContext>>) -> AreaGraph {
        // gross but allows for neater tests
        let world = world_from_chunks_blocking(chunks).into_inner();
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
                .set_block((CHUNK_SIZE.as_i32() - 1, 5, 0), DummyBlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 5, 0), DummyBlockType::Grass)
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
                .set_block((15, 0, 0), DummyBlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 0, 0), DummyBlockType::Grass)
                .build((1, 0)),
            ChunkBuilder::new()
                .set_block((15, 15, 0), DummyBlockType::Grass)
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
        // logging::for_tests();

        // step up from chunk 0 (0,5,N-1) to chunk 1 (-1, 5, N) slab 1
        let graph = make_graph(vec![
            ChunkBuilder::new()
                .set_block((0, 5, -1), DummyBlockType::Grass)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((CHUNK_SIZE.as_i32() - 1, 5, 0), DummyBlockType::Stone)
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
        // logging::for_tests();

        // -1, 5, 16 -> 0, 5, 15
        let graph = make_graph(vec![
            ChunkBuilder::new()
                .set_block((0, 5, SLAB_SIZE.as_i32() - 1), DummyBlockType::Grass)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block(
                    (CHUNK_SIZE.as_i32() - 1, 5, SLAB_SIZE.as_i32()),
                    DummyBlockType::Stone,
                )
                .build((-1, 0)),
        ]);

        assert_eq!(graph.node_count(), 2); // 1 area in (0,0) and 1 in (-1,0)
        assert_eq!(graph.edge_count(), 2); // 1 each way

        let a = WorldArea {
            chunk: ChunkLocation(0, 0),
            slab: 1.into(),
            area: SlabAreaIndex::FIRST,
        };
        let b = WorldArea {
            chunk: ChunkLocation(-1, 0),
            slab: 1.into(),
            area: SlabAreaIndex::FIRST,
        };

        let _ = get_edge(&graph, a, b).expect("edge should exist");
        let _ = get_edge(&graph, b, a).expect("node should exist both ways");
    }
    #[test]
    fn empty_slab_no_areas() {
        // logging::for_tests();

        let graph = make_graph(vec![ChunkBuilder::new()
            // 1 block in second slab
            .set_block((2, 2, SLAB_SIZE.as_i32()), DummyBlockType::Stone)
            .build((0, 0))]);

        // BUG: area is not created because no neighbouring chunks found
        assert_eq!(graph.node_count(), 1); // just one area in slab idx 1
    }

    #[test]
    fn full_slices_flat() {
        let chunks = vec![
            ChunkBuilder::new()
                .fill_slice(0, DummyBlockType::Stone)
                .build((0, 0)),
            ChunkBuilder::new()
                .fill_slice(0, DummyBlockType::Stone)
                .build((1, 0)),
            ChunkBuilder::new()
                .fill_slice(0, DummyBlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(0, DummyBlockType::Stone)
                .build((0, 1)),
            ChunkBuilder::new()
                .fill_slice(0, DummyBlockType::Stone)
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
                .filter(|(_, (a, b))| graph.graph[*a].0.chunk == ChunkLocation(0, 0)
                    || graph.graph[*b].0.chunk == ChunkLocation(0, 0))
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
                exit: BlockPosition::new_unchecked(0, 0, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::Walk,
                width: 3,
                exit: BlockPosition::new_unchecked(0, 4, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpUp,
                width: 2,
                exit: BlockPosition::new_unchecked(0, 7, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpUp,
                width: 1,
                exit: BlockPosition::new_unchecked(0, 10, GlobalSliceIndex::new(0)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpUp,
                width: 1,
                exit: BlockPosition::new_unchecked(0, 11, GlobalSliceIndex::new(5)),
                direction,
            },
            AreaNavEdge {
                cost: EdgeCost::JumpDown,
                width: 1,
                exit: BlockPosition::new_unchecked(0, 12, GlobalSliceIndex::new(5)),
                direction,
            },
        ];

        assert_eq!(ports, expected);
    }

    #[test]
    fn world_port_discovery() {
        let graph = make_graph(vec![
            ChunkBuilder::new()
                .fill_slice(3, DummyBlockType::Stone)
                .build((-4, -4)),
            ChunkBuilder::new()
                // 3 wide port
                .set_block((0, 5, 3), DummyBlockType::Grass)
                .set_block((0, 6, 3), DummyBlockType::Grass)
                .set_block((0, 7, 3), DummyBlockType::Grass)
                // little bridge to have 1 connected area
                .set_block((1, 7, 3), DummyBlockType::Stone)
                .set_block((1, 8, 3), DummyBlockType::Stone)
                .set_block((1, 9, 4), DummyBlockType::Stone)
                .set_block((1, 10, 4), DummyBlockType::Stone)
                // another disconnected 1 wide port
                .set_block((0, 10, 4), DummyBlockType::Grass)
                .build((-3, -4)),
            ChunkBuilder::new().build((0, 0)),
        ]);

        let mut edges = graph.get_edges(WorldArea::new((-4, -4)), WorldArea::new((-3, -4)));

        let mut expected = vec![
            AreaNavEdge {
                direction: NeighbourOffset::East,
                cost: EdgeCost::Walk,
                exit: (15, 5, 4).try_into().unwrap(),
                width: 3,
            },
            AreaNavEdge {
                direction: NeighbourOffset::East,
                cost: EdgeCost::JumpUp,
                exit: (15, 10, 4).try_into().unwrap(),
                width: 1,
            },
        ];

        edges.sort_by_key(|e| e.exit.y());
        expected.sort_by_key(|e| e.exit.y());

        assert_eq!(edges, expected);
    }

    #[test]
    fn area_path_ring_all_directions() {
        // logging::for_tests();

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
                    &AreaGraph::search_context(),
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
                        exit: (3, 0, 301).try_into().unwrap(),
                        width: 1,
                    },
                ),
                // east
                AreaPathNode::new(
                    WorldArea::new_with_slab((0, 0), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::East,
                        cost: EdgeCost::JumpDown,
                        exit: (CHUNK_SIZE.as_i32() - 1, 3, 302).try_into().unwrap(),
                        width: 1,
                    },
                ),
                // north
                AreaPathNode::new(
                    WorldArea::new_with_slab((0, 1), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::North,
                        cost: EdgeCost::JumpUp,
                        exit: (3, CHUNK_SIZE.as_i32() - 1, 301).try_into().unwrap(),
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
                    &AreaGraph::search_context(),
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
                        exit: (3, 0, 302).try_into().unwrap(),
                        width: 1,
                    },
                ),
                // west
                AreaPathNode::new(
                    WorldArea::new_with_slab((-1, 0), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::West,
                        cost: EdgeCost::JumpUp,
                        exit: (0, 3, 301).try_into().unwrap(),
                        width: 1,
                    },
                ),
                // north
                AreaPathNode::new(
                    WorldArea::new_with_slab((-1, 1), SLAB),
                    AreaNavEdge {
                        direction: NeighbourOffset::North,
                        cost: EdgeCost::JumpDown,
                        exit: (3, CHUNK_SIZE.as_i32() - 1, 302).try_into().unwrap(),
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
                .set_block((14, 2, 1), DummyBlockType::Stone)
                .set_block((14, 3, 1), DummyBlockType::Stone)
                .set_block((15, 2, 2), DummyBlockType::Stone)
                .set_block((15, 3, 2), DummyBlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(3, DummyBlockType::Grass)
                .build((0, 0)),
            ChunkBuilder::new()
                // 1 wide port still going east
                .set_block((0, 5, 2), DummyBlockType::Stone)
                .set_block((1, 5, 1), DummyBlockType::Stone)
                .build((1, 0)),
        ]);
        let path = graph
            .find_area_path(
                WorldArea::new((-1, 0)),
                WorldArea::new((1, 0)),
                &AreaGraph::search_context(),
            )
            .expect("path should succeed");

        let expected = vec![
            AreaPathNode::new_start(WorldArea::new((-1, 0))),
            AreaPathNode::new(
                WorldArea::new((0, 0)),
                AreaNavEdge {
                    direction: NeighbourOffset::East,
                    cost: EdgeCost::JumpUp,
                    exit: (15, 2, 3).try_into().unwrap(),
                    width: 2,
                },
            ),
            AreaPathNode::new(
                WorldArea::new((1, 0)),
                AreaNavEdge {
                    direction: NeighbourOffset::East,
                    cost: EdgeCost::JumpDown,
                    exit: (15, 5, 4).try_into().unwrap(),
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

        let w = world_from_chunks_blocking(vec![
            ChunkBuilder::new()
                .set_block((14, 2, 201), DummyBlockType::Stone)
                .set_block((15, 2, 201), DummyBlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(201, DummyBlockType::Grass)
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
                    exit: (15, 2, 202).try_into().unwrap(),
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

        let w = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(201, DummyBlockType::Grass)
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
            .fill_slice(1, DummyBlockType::Grass)
            .build((0, 0))]);

        let err = graph.find_area_path(
            WorldArea::new((0, 0)),
            WorldArea::new((100, 20)),
            &AreaGraph::search_context(),
        );

        assert!(matches!(err, Err(AreaPathError::NoSuchNode(_))));
    }

    #[test]
    fn area_edge_reverse() {
        let edge = AreaNavEdge {
            direction: NeighbourOffset::South,
            cost: EdgeCost::JumpUp,
            exit: (5, 0, 5).try_into().unwrap(),
            width: 2,
        };

        let reversed = AreaNavEdge {
            direction: NeighbourOffset::North,
            cost: EdgeCost::JumpDown,
            exit: BlockPosition::new_unchecked(
                5,
                CHUNK_SIZE.as_block_coord() - 1,
                GlobalSliceIndex::new(6),
            ),
            width: 2,
        };

        assert_eq!(edge.reversed(), reversed);
        assert_eq!(reversed.reversed(), edge);
    }

    #[test]
    fn port_exit_closest() {
        assert_eq!(
            AreaNavEdge {
                direction: NeighbourOffset::South,
                cost: EdgeCost::Walk,
                exit: (4, 4, 4).try_into().unwrap(),
                width: 1
            }
            .exit_closest((10, 10, 4).try_into().unwrap()), // doesn't matter, there is only 1 candidate
            (4, 4, 4).try_into().unwrap()
        );

        let edge = AreaNavEdge {
            direction: NeighbourOffset::South,
            cost: EdgeCost::Walk,
            exit: (4, 4, 4).try_into().unwrap(), // [4, 8] in x axis
            width: 5,
        };

        assert_eq!(
            edge.exit_closest((2, 0, 4).try_into().unwrap()),
            (4, 4, 4).try_into().unwrap()
        );
        assert_eq!(
            edge.exit_closest((6, 0, 4).try_into().unwrap()),
            (6, 4, 4).try_into().unwrap()
        );
        assert_eq!(
            edge.exit_closest((15, 0, 4).try_into().unwrap()),
            (8, 4, 4).try_into().unwrap()
        );
    }
}
