use std::cell::{Cell, RefCell};
use std::fmt::{Display, Error, Formatter};

use cgmath::MetricSpace;
use cgmath::Vector3;
use petgraph::algo::astar;
use petgraph::graph::EdgeReference;
use petgraph::prelude::*;
use petgraph::{Graph, Undirected};

use crate::chunk::Chunk;
use crate::coordinate::world::{Block, CHUNK_SIZE};
use crate::grid::{CoordType, Grid, GridImpl};
use crate::SliceRange;
use crate::{grid_declare, ChunkGrid};

type NavIdx = u32;
type NavGraph = Graph<Node, Edge, Undirected, NavIdx>; // TODO directed

pub struct Navigation {
    graph: NavGraph,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Node(Block);

type Edge = ();

//#[derive(Copy, Clone, PartialEq, Eq, Debug)]
//struct Edge(i32); // cost

impl Navigation {
    /// Initialize from a chunk's geometry
    pub fn from_chunk(chunk: &ChunkGrid) -> Self {
        let graph = RefCell::new(NavGraph::new_undirected());

        #[derive(Default)]
        pub struct NavMarker {
            done: Cell<bool>,
            node: Cell<Option<NodeIndex>>,
            solid: bool,
        }

        const SZ: usize = CHUNK_SIZE as usize;
        grid_declare!(struct NavCreationGrid<NavCreationGridImpl, NavMarker>, SZ, SZ, SZ);

        // instead of constantly reading from the chunk terrain, read it once and populate
        // this 3d bitmask
        let mut grid = NavCreationGrid::default();

        // populate grid with solidness flag
        for idx in grid.indices() {
            let pos = grid.unflatten_index(idx);
            let mut marker = &mut grid[idx];
            marker.solid = chunk[&pos].solid();
        }

        // no corners
        const NEIGHBOURS: [(i32, i32); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];

        let walkable = |pos: Block| -> bool {
            let (x, y, z) = pos.into();

            // nothing below or above: nope
            if z == 0 || z >= (CHUNK_SIZE - 1) as i32 {
                return false;
            }

            // solid: nope
            let coord: CoordType = pos.into();
            let marker = &grid[&coord];
            if marker.solid {
                return false;
            }

            // below not solid either: nope
            let below: Block = (x, y, z - 1).into();
            if !grid[&below.into()].solid {
                return false;
            }

            // nice
            true
        };

        let get_or_create_node = |pos: Block| -> NodeIndex {
            let coord: CoordType = pos.into();
            match grid[&coord].node.get() {
                None => {
                    let node = graph.borrow_mut().add_node(Node(pos));
                    grid[&coord].node.set(Some(node));
                    node
                }
                Some(node) => node,
            }
        };

        let add_edge = |a: NodeIndex, b: NodeIndex| {
            let (from, to) = if a < b { (a, b) } else { (b, a) };

            graph.borrow_mut().update_edge(from, to, ());
        };

        fn get_neighbour(src: (u16, u16), delta: (i32, i32)) -> Option<(u16, u16)> {
            fn add(u: u16, i: i32) -> Option<u16> {
                if i.is_negative() {
                    u.checked_sub(i.wrapping_abs() as u16)
                } else {
                    u.checked_add(i as u16)
                }
            }

            const LIMIT: u16 = (CHUNK_SIZE as u16) - 1;
            match (add(src.0, delta.0), add(src.1, delta.1)) {
                (Some(nx), Some(ny)) if nx <= LIMIT && ny <= LIMIT => Some((nx, ny)),
                _ => None,
            }
        }

        // flood fill from every block
        for idx in grid.indices() {
            if grid[idx].done.get() {
                continue;
            }
            grid[idx].done.set(true);

            let pos = grid.unflatten_index(idx);
            let pos: Block = (&pos).into();
            if !walkable(pos) {
                // not suitable
                continue;
            }
            // suitable
            let node = get_or_create_node(pos);
            grid[idx].node.set(Some(node));
            let (x, y, z) = pos.into();

            // horizontal neighbours
            for &(dx, dy) in &NEIGHBOURS {
                let (nx, ny) = match get_neighbour((x, y), (dx, dy)) {
                    Some((x, y)) => (x, y),
                    _ => continue,
                };

                let neighbour: Block = (nx, ny, z).into();
                if walkable(neighbour) {
                    // nice, walkable
                    // connect with parent
                    let node_neighbour = get_or_create_node(neighbour);
                    add_edge(node, node_neighbour);
                } else {
                    // maybe above is walkable instead
                    let above: Block = (nx, ny, z + 1).into();
                    if walkable(above) {
                        let node_above = get_or_create_node(above);
                        add_edge(node, node_above);
                        // graph.add_edge(node_above, node, ());
                    }
                }
            }
        }

        Self {
            graph: graph.into_inner(),
        }
    }

    pub fn resolve_node(&self, block: Block) -> Option<NodeIndex> {
        self.graph
            .node_indices()
            .find(|i| self.graph.node_weight(*i).unwrap().0 == block)
    }

    // TODO return Result instead
    pub fn find_path<F: Into<Block>, T: Into<Block>>(&self, from: F, to: T) -> Option<Path> {
        // TODO better lookup
        let from: Block = from.into();
        let to: Block = to.into();

        let (from_node, to_node) = match (self.resolve_node(from), self.resolve_node(to)) {
            (Some(from), Some(to)) => (from, to),
            _ => return None,
        };

        let to_vec: Vector3<f32> = to.to_chunk_point_centered().into();

        match astar(
            &self.graph,
            from_node,
            |n| n == to_node,
            |_| 1, // edge cost
            |n| {
                let node = self.graph.node_weight(n).unwrap();
                let here_vec: Vector3<f32> = node.0.to_chunk_point_centered().into();
                to_vec.distance2(here_vec) as i32
            },
        ) {
            Some((_cost, nodes)) => {
                let points = nodes
                    .iter()
                    .map(|nid| self.graph.node_weight(*nid).unwrap().0)
                    .collect();
                Some(Path(points))
            }
            _ => None,
        }
    }

    /// Called when the chunk geometry changed, updates internal graph
    pub fn on_chunk_update(&mut self, _chunk: &mut Chunk) {
        // TODO
    }

    pub fn nodes(&self) -> impl Iterator<Item = NodeIndex> {
        self.graph.node_indices()
    }

    pub fn node_position(&self, idx: NodeIndex) -> &Block {
        &self.graph.node_weight(idx).expect("bad node index").0
    }

    pub fn edges_for_node(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (EdgeReference<Edge, NavIdx>, NodeIndex, NodeIndex)> {
        self.graph.edges(node).map(|e| (e, e.source(), e.target()))
    }

    pub fn all_edges(
        &self,
    ) -> impl Iterator<Item = (EdgeReference<Edge, NavIdx>, NodeIndex, NodeIndex)> {
        self.graph
            .edge_references()
            .map(|e| (e, e.source(), e.target()))
    }

    pub fn is_visible(&self, node: NodeIndex, range: SliceRange) -> bool {
        let Block(_, _, slice) = self.graph.node_weight(node).unwrap().0;
        range.contains(slice - 1) // -1 to only render if the supporting block below is visible
    }
}

#[derive(Debug)]
pub struct Path(Vec<Block>);

impl Display for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(
            f,
            "Path({:?})",
            self.0.iter().map(|b| b.flatten()).collect::<Vec<_>>()
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::ChunkBuilder;
    use crate::CHUNK_SIZE;

    #[test]
    fn simple() {
        let chunk = ChunkBuilder::new()
            .set_block((1, 1, 1), BlockType::Dirt)
            .set_block((1, 2, 1), BlockType::Dirt)
            .build((0, 0));

        let nav = chunk.navigation();

        // only 1 on top of each block
        assert_eq!(nav.graph.node_count(), 2);
        assert_eq!(nav.graph.edge_count(), 1); // 1 edge because undirected
    }

    #[test]
    fn step_up() {
        let chunk = ChunkBuilder::new()
            .set_block((0, 0, 0), BlockType::Dirt)
            .set_block((1, 0, 0), BlockType::Dirt)
            .set_block((1, 1, 0), BlockType::Dirt)
            .set_block((0, 1, 0), BlockType::Dirt)
            .set_block((1, 1, 1), BlockType::Grass)
            .build((0, 0));

        let nav = chunk.navigation();

        // only 1 on top of each block
        assert_eq!(nav.graph.node_count(), 4);
        assert_eq!(nav.graph.edge_count(), 4); // all should be interconnected
    }

    #[test]
    fn impossible_step_up() {
        let chunk = ChunkBuilder::new()
            .set_block((1, 1, 1), BlockType::Dirt)
            .set_block((1, 2, 3), BlockType::Dirt)
            .build((0, 0));

        let nav = chunk.navigation();

        assert_eq!(nav.graph.node_count(), 2);
        assert_eq!(nav.graph.edge_count(), 0); // unconnected because out of reach
    }

    #[test]
    fn simple_path() {
        let chunk = ChunkBuilder::new()
            .set_block((1, 1, 1), BlockType::Dirt)
            .set_block((1, 2, 1), BlockType::Dirt)
            .set_block((1, 3, 1), BlockType::Dirt)
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((1, 1, 2), (1, 3, 2)).expect("path succeeded");
        assert_eq!(
            path.0.iter().map(|b| b.flatten()).collect::<Vec<_>>(),
            vec![(1, 1, 2), (1, 2, 2), (1, 3, 2)]
        );
    }

    #[test]
    fn null_path() {
        let chunk = ChunkBuilder::new()
            .set_block((1, 1, 1), BlockType::Dirt)
            .set_block((1, 2, 1), BlockType::Dirt)
            .set_block((1, 3, 1), BlockType::Dirt)
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((3, 3, 7), (4, 2, 12));
        assert!(path.is_none());

        let path = nav.find_path((1, 1, 2), (9, 9, 9));
        assert!(path.is_none());

        let path = nav.find_path((9, 9, 9), (1, 3, 2));
        assert!(path.is_none());
    }

    #[test]
    fn no_wrapping() {
        let chunk = ChunkBuilder::new()
            .fill_slice(0, BlockType::Stone) // 0 is filled and walkable
            .build((0, 0));

        let nav = chunk.navigation();

        let node = nav.resolve_node((0, 1, 1).into()).unwrap();
        let far_far_away = nav.resolve_node((15, 0, 1).into()).unwrap();

        // there should definitely not be a edge between these 2
        for (_, s, t) in nav.edges_for_node(node) {
            assert_ne!(s, far_far_away);
            assert_ne!(t, far_far_away);
        }
    }

    #[test]
    fn complex_path() {
        let chunk = ChunkBuilder::new()
            .fill_slice(0, BlockType::Stone) // 0 is filled and walkable
            .fill_range((3, 0, 0), (4, 4, 4), |_| Some(BlockType::Dirt)) // big wall at x=3
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((0, 0, 1), (8, 0, 1));
        assert!(path.is_some());

        let chunk = ChunkBuilder::new()
            .fill_slice(0, BlockType::Stone) // 0 is filled and walkable
            .fill_range((3, 0, 0), (4, CHUNK_SIZE as u16 + 1, 4), |_| Some(BlockType::Dirt)) // impassible wall at x=3
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((0, 0, 1), (8, 0, 1));
        assert!(path.is_none());
    }
}
