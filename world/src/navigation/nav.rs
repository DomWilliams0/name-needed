use std::cell::{Cell, RefCell};
use std::fmt::{Display, Error, Formatter};

use cgmath::MetricSpace;
use cgmath::Vector3;
use petgraph::algo::astar;
use petgraph::graph::EdgeReference;
use petgraph::visit::EdgeRef;

use crate::block::{Block, BlockHeight};
use crate::chunk::{Chunk, CHUNK_DEPTH, CHUNK_SIZE};
use crate::coordinate::world::BlockPosition;
use crate::grid::{CoordType, Grid, GridImpl};
use crate::navigation::graph::{Edge, NavGraph, NavIdx};
use crate::navigation::{Node, NodeIndex};
use crate::SliceRange;
use crate::{grid_declare, ChunkGrid};

pub struct Navigation {
    graph: NavGraph,
}

impl Navigation {
    /// Initialize from a chunk's geometry
    pub fn from_chunk(chunk: &ChunkGrid) -> Self {
        let graph = RefCell::new(NavGraph::new_undirected());

        #[derive(Default)]
        pub struct NavMarker {
            done: Cell<bool>,
            node: Cell<Option<NodeIndex>>,
            /// is material solid
            solid: bool,

            /// is not a half step
            height: BlockHeight,
        }

        grid_declare!(struct NavCreationGrid<NavCreationGridImpl, NavMarker>, CHUNK_SIZE.as_usize(), CHUNK_SIZE.as_usize(), CHUNK_DEPTH.as_usize());

        // instead of constantly reading from the chunk terrain, read it once and populate
        // this 3d bitmask
        let mut grid = NavCreationGrid::default();

        // populate grid with solidness flag
        for idx in grid.indices() {
            let pos = grid.unflatten_index(idx);
            let mut marker: &mut NavMarker = &mut grid[idx];
            let block: Block = chunk[&pos];
            marker.solid = block.solid();
            marker.height = block.block_height();
        }

        // no corners
        const NEIGHBOURS: [(i32, i32); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];

        let walkable = |pos: BlockPosition| -> bool {
            let (x, y, z) = pos.into();

            // nothing below or above: nope
            if z == 0 || z >= (CHUNK_DEPTH.as_i32() - 1) {
                return false;
            }

            let coord: CoordType = pos.into();
            let marker: &NavMarker = &grid[&coord];

            if marker.solid {
                // solid and half block: yes
                // solid and full block: no
                return !marker.height.solid();
            }

            let below: BlockPosition = (x, y, z - 1).into();
            let below: &NavMarker = &grid[&below.into()];

            // below not solid either: nope
            if !below.solid {
                return false;
            }

            // below is solid but half block: nope
            if !below.height.solid() {
                return false;
            }

            // below is solid and full: nice
            true
        };

        let get_or_create_node = |pos: BlockPosition| -> NodeIndex {
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

            let edge = {
                let g = graph.borrow();
                let from_pos: BlockPosition = g.node_weight(from).unwrap().0;
                let to_pos: BlockPosition = g.node_weight(to).unwrap().0;

                let from_height: BlockHeight = grid[&from_pos.into()].height;
                let to_height: BlockHeight = grid[&to_pos.into()].height;

                if from_height.solid() && to_height.solid() {
                    // both are solid
                    if from_pos.2 == to_pos.2 {
                        // and on the same level, must be a ezpz flat walk
                        Edge::Walk(BlockHeight::Full)
                    } else {
                        // but not on the same level, must be a jump
                        Edge::Jump
                    }
                } else {
                    // one is elevated, so its a climb
                    // choose the minimum i.e. the non-full step
                    let height = std::cmp::min(from_height, to_height);
                    Edge::Walk(height)
                }
            };
            graph.borrow_mut().update_edge(from, to, edge);
        };

        fn get_neighbour(src: (u16, u16), delta: (i32, i32)) -> Option<(u16, u16)> {
            fn add(u: u16, i: i32) -> Option<u16> {
                if i.is_negative() {
                    u.checked_sub(i.wrapping_abs() as u16)
                } else {
                    u.checked_add(i as u16)
                }
            }

            const X_LIMIT: u16 = (CHUNK_SIZE.as_u16()) - 1;
            const Y_LIMIT: u16 = (CHUNK_SIZE.as_u16()) - 1;
            match (add(src.0, delta.0), add(src.1, delta.1)) {
                (Some(nx), Some(ny)) if nx <= X_LIMIT && ny <= Y_LIMIT => Some((nx, ny)),
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
            let pos: BlockPosition = (&pos).into();
            if !walkable(pos) {
                // not suitable
                continue;
            }
            // suitable
            let node = get_or_create_node(pos);
            grid[idx].node.set(Some(node));
            let (x, y, z) = pos.into();

            // if above is blocked, dont bother checking for diagonal edges
            let above_blocked = {
                let above: BlockPosition = (x, y, z + 1).into();
                grid[&above.into()].solid
            };

            // horizontal neighbours
            for &(dx, dy) in &NEIGHBOURS {
                let (nx, ny) = match get_neighbour((x, y), (dx, dy)) {
                    Some((x, y)) => (x, y),
                    _ => continue,
                };

                let neighbour: BlockPosition = (nx, ny, z).into();
                if walkable(neighbour) {
                    // nice, walkable
                    // connect with parent
                    let node_neighbour = get_or_create_node(neighbour);
                    add_edge(node, node_neighbour);
                } else if !above_blocked {
                    // maybe above is walkable instead, either via a jump or step
                    let above: BlockPosition = (nx, ny, z + 1).into();
                    if walkable(above) {
                        let node_above = get_or_create_node(above);
                        add_edge(node, node_above);
                    }
                }
            }
        }

        // remove unconnected nodes (disable in tests)
        if !cfg!(test) {
            graph
                .borrow_mut()
                .retain_nodes(|g, n| g.edges(n).count() > 0);
        }

        Self {
            graph: graph.into_inner(),
        }
    }

    pub fn resolve_node(&self, block: BlockPosition) -> Option<NodeIndex> {
        self.graph
            .node_indices()
            .find(|i| self.graph.node_weight(*i).unwrap().0 == block)
    }

    // TODO return Result instead
    pub fn find_path<F: Into<BlockPosition>, T: Into<BlockPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Option<Path> {
        // TODO better lookup
        let from: BlockPosition = from.into();
        let to: BlockPosition = to.into();

        let (from_node, to_node) = match (self.resolve_node(from), self.resolve_node(to)) {
            (Some(from), Some(to)) => (from, to),
            _ => return None,
        };

        let to_vec: Vector3<f32> = to.to_chunk_point_centered().into();

        match astar(
            &self.graph,
            from_node,
            |n| n == to_node,
            |e| e.weight().weight(),
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

    pub fn node_position(&self, idx: NodeIndex) -> &BlockPosition {
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
        let BlockPosition(_, _, slice) = self.graph.node_weight(node).unwrap().0;
        range.contains(slice - 1) // -1 to only render if the supporting block below is visible
    }
}

#[derive(Debug)]
pub struct Path(Vec<BlockPosition>);

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
    use crate::block::{BlockHeight, BlockType};
    use crate::chunk::ChunkBuilder;
    use crate::navigation::graph::Edge;
    use crate::CHUNK_SIZE;

    #[test]
    fn simple() {
        // just 1 at 0,0,0
        let chunk = ChunkBuilder::new()
            .set_block((0, 0, 0), BlockType::Dirt)
            .build((0, 0));

        let nav = chunk.navigation();
        assert_eq!(nav.graph.node_count(), 1);

        // just 1 at 0,0,1
        let chunk = ChunkBuilder::new()
            .set_block((0, 0, 1), BlockType::Dirt)
            .build((0, 0));

        let nav = chunk.navigation();
        assert_eq!(nav.graph.node_count(), 1);

        // 2 adjacent on level 1
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
    fn jump_up() {
        //  _
        // _
        let chunk = ChunkBuilder::new()
            .set_block((1, 0, 0), BlockType::Dirt)
            .set_block((1, 1, 1), BlockType::Grass)
            .build((0, 0));

        let nav = chunk.navigation();

        assert_eq!(nav.graph.node_count(), 2); // only 1 on top of each block
        assert_eq!(nav.graph.edge_count(), 1); // 1 jump edge
        assert_eq!(
            nav.graph.edge_references().next().unwrap().weight(),
            &Edge::Jump
        );
    }

    #[test]
    fn step_up_single() {
        // single step up
        // _-
        let chunk = ChunkBuilder::new()
            .set_block((1, 0, 0), BlockType::Dirt)
            .set_block((1, 1, 1), (BlockType::Grass, BlockHeight::Half))
            .build((0, 0));

        let nav = chunk.navigation();

        assert_eq!(nav.graph.node_count(), 2); // only 1 on top of each block
        assert_eq!(nav.graph.edge_count(), 1); // can step on half block
        assert_eq!(
            nav.graph.edge_references().next().unwrap().weight(),
            &Edge::Walk(BlockHeight::Half)
        );
    }

    #[test]
    fn step_up_full() {
        // full step up
        //    _
        // _--
        let chunk = ChunkBuilder::new()
            .set_block((1, 0, 0), BlockType::Dirt)
            .set_block((1, 1, 1), (BlockType::Grass, BlockHeight::Half))
            .set_block((1, 2, 1), BlockType::Grass)
            .build((0, 0));

        let nav = chunk.navigation();

        assert_eq!(nav.graph.node_count(), 3); // only 1 on top of each block
        assert_eq!(nav.graph.edge_count(), 2); // can step on half block

        // both edges should be half steps
        let mut edges = nav.graph.edge_references();
        assert_eq!(
            edges.next().unwrap().weight(),
            &Edge::Walk(BlockHeight::Half)
        );
        assert_eq!(
            edges.next().unwrap().weight(),
            &Edge::Walk(BlockHeight::Half)
        );
    }

    #[test]
    fn impossible_jump_up() {
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
        let path = nav.find_path((1, 1, 2), (1, 3, 2));
        assert!(path.is_some());
        assert_eq!(
            path.unwrap()
                .0
                .iter()
                .map(|b| b.flatten())
                .collect::<Vec<_>>(),
            vec![(1, 1, 2), (1, 2, 2), (1, 3, 2)]
        );
    }

    #[test]
    fn simple_path_up_some_stairs() {
        let chunk = ChunkBuilder::new()
            .set_block((0, 0, 0), (BlockType::Dirt, BlockHeight::Full))
            .set_block((0, 1, 1), (BlockType::Dirt, BlockHeight::Half))
            .set_block((0, 2, 1), (BlockType::Dirt, BlockHeight::Full))
            .set_block((0, 3, 2), (BlockType::Dirt, BlockHeight::Half))
            .set_block((0, 4, 2), (BlockType::Dirt, BlockHeight::Full))
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((0, 0, 1), (0, 4, 3));
        assert!(path.is_some());
        assert_eq!(
            path.unwrap()
                .0
                .iter()
                .map(|b| b.flatten())
                .collect::<Vec<_>>(),
            vec![(0, 0, 1), (0, 1, 1), (0, 2, 2), (0, 3, 2), (0, 4, 3)]
        );
    }

    #[test]
    fn null_path() {
        let chunk = ChunkBuilder::new()
            .set_block((1, 1, 1), BlockType::Dirt)
            .set_block((1, 2, 1), BlockType::Dirt)
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
            .fill_range((3, 0, 0), (4, 4, 4), |_| BlockType::Dirt) // big wall at x=3
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((0, 0, 1), (8, 0, 1));
        assert!(path.is_some());

        let chunk = ChunkBuilder::new()
            .fill_slice(0, BlockType::Stone) // 0 is filled and walkable
            .fill_range((3, 0, 0), (4, CHUNK_SIZE.as_u16() + 1, 4), |_| BlockType::Dirt) // impassible wall at x=3
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((0, 0, 1), (8, 0, 1));
        assert!(path.is_none());
    }

    #[test]
    fn path_costs() {
        let chunk = ChunkBuilder::new()
            .fill_slice(0, BlockType::Stone) // 0 is filled and walkable
            .set_block((1, 0, 1), BlockType::Dirt) // little lump
            .set_block((2, 0, 1), BlockType::Dirt)
            .set_block((2, 0, 2), BlockType::Dirt)
            .set_block((3, 0, 1), BlockType::Dirt)
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((0, 0, 1), (4, 0, 1));

        // path should take into account that jumps cost more than walking, so should walk around
        // the obstruction rather than climbing up and down it
        assert!(path.is_some());
        assert!(path.unwrap().0.into_iter().all(|p| (p.2).0 == 1));
    }

    #[test]
    fn dont_bang_your_head() {
        let chunk = ChunkBuilder::new()
            .set_block((0, 0, 1), BlockType::Dirt)  // start here, fine
            .set_block((1, 0, 0), BlockType::Dirt)  // step down, fine
            .set_block((1, 0, 2), BlockType::Stone) // blocks your head!!!!
            .build((0, 0));

        let nav = chunk.navigation();
        let path = nav.find_path((0, 0, 2), (1, 0, 1));
        assert!(path.is_none());
    }
}
