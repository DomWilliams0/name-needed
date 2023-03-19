use crate::navigationv2::{ChunkArea, SlabArea, SlabNavEdge};
use misc::{trace, Itertools};
use petgraph::stable_graph::*;
use std::collections::HashMap;
use unit::world::{ChunkLocation, SlabLocation};

/// Area within the world
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct WorldArea {
    pub chunk_idx: ChunkLocation,
    pub chunk_area: ChunkArea,
}

type WorldNavGraphType = StableUnGraph<WorldArea, SlabNavEdge, u32>;

#[derive(Default)]
pub struct WorldGraph {
    graph: WorldNavGraphType,
    nodes: HashMap<WorldArea, NodeIndex>,
}

impl WorldGraph {
    pub fn add_inter_slab_edges(
        &mut self,
        from: SlabLocation,
        to: SlabLocation,
        edges: impl Iterator<Item = (SlabArea, SlabArea, SlabNavEdge)>,
    ) {
        for (a, b, e) in edges {
            let src = self.add_node(WorldArea::from((from, a)));
            let dst = self.add_node(WorldArea::from((to, b)));

            // old edges should have been cleared already
            debug_assert!(!self.graph.contains_edge(src, dst));

            self.graph.add_edge(src, dst, e);
        }
    }

    fn add_node(&mut self, area: WorldArea) -> NodeIndex {
        *self.nodes.entry(area).or_insert_with(|| {
            debug_assert!(!self.graph.node_weights().contains(&area), "duplicate area");
            self.graph.add_node(area)
        })
    }
}

impl From<(SlabLocation, SlabArea)> for WorldArea {
    fn from((slab, area): (SlabLocation, SlabArea)) -> Self {
        WorldArea {
            chunk_idx: slab.chunk,
            chunk_area: ChunkArea {
                slab_idx: slab.slab,
                slab_area: area,
            },
        }
    }
}
