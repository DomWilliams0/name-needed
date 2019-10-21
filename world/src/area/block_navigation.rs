//! Navigation inside an area

use std::collections::HashMap;

use cgmath::MetricSpace;
use cgmath::Vector3;

use petgraph::algo::astar;
use petgraph::graph::NodeIndex;

use crate::area::path::BlockPath;
use crate::area::EdgeCost;
use crate::coordinate::world::ChunkPoint;
use crate::BlockPosition;
use petgraph::prelude::DiGraph;

pub type BlockGraphType = DiGraph<Node, Edge>;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Node(pub BlockPosition);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Edge(pub EdgeCost);

pub struct BlockGraph {
    graph: BlockGraphType,
    node_lookup: HashMap<BlockPosition, NodeIndex>,
}

impl BlockGraph {
    pub fn new() -> Self {
        Self {
            graph: BlockGraphType::new(),
            node_lookup: HashMap::new(),
        }
    }

    fn get_node_or_create(&mut self, pos: BlockPosition) -> NodeIndex {
        if let Some(idx) = self.node_lookup.get(&pos) {
            *idx
        } else {
            let idx = self.graph.add_node(Node(pos));
            self.node_lookup.insert(pos, idx);
            idx
        }
    }

    pub fn add_edge<F, T>(&mut self, from: F, to: T, cost: EdgeCost)
    where
        F: Into<BlockPosition>,
        T: Into<BlockPosition>,
    {
        let from = self.get_node_or_create(from.into());
        let to = self.get_node_or_create(to.into());
        self.graph.update_edge(from, to, Edge(cost));

        self.graph.update_edge(to, from, Edge(cost.opposite()));
    }

    pub fn get_edge_between<F, T>(&self, from: F, to: T) -> Option<EdgeCost>
    where
        F: Into<BlockPosition>,
        T: Into<BlockPosition>,
    {
        let from = self.get_node(from.into());
        let to = self.get_node(to.into());
        if let (Some(from), Some(to)) = (from, to) {
            self.graph
                .find_edge(from, to)
                .and_then(|e| self.graph.edge_weight(e))
                .map(|e| e.0)
        } else {
            None
        }
    }

    pub fn graph(&self) -> &BlockGraphType {
        &self.graph
    }

    fn get_node(&self, pos: BlockPosition) -> Option<NodeIndex> {
        self.node_lookup.get(&pos).copied()
    }

    pub(crate) fn find_path<F: Into<BlockPosition>, T: Into<BlockPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Option<BlockPath> {
        let from: BlockPosition = from.into();
        let to: BlockPosition = to.into();

        let (from_node, to_node) = match (self.get_node(from), self.get_node(to)) {
            (Some(from), Some(to)) => (from, to),
            _ => return None,
        };

        let to_vec: Vector3<f32> = ChunkPoint::from(to).into();

        match astar(
            &self.graph,
            from_node,
            |n| n == to_node,
            |e| e.weight().0.weight(),
            |n| {
                let node = self.graph.node_weight(n).unwrap();
                let here_vec: Vector3<f32> = ChunkPoint::from(node.0).into();
                to_vec.distance2(here_vec) as i32
            },
        ) {
            Some((_cost, nodes)) => {
                // collect block positions with default edge cost
                let mut points: Vec<(BlockPosition, EdgeCost)> = nodes
                    .iter()
                    .map(|nid| (self.graph.node_weight(*nid).unwrap().0, EdgeCost::Walk))
                    .collect();

                // populate edge costs using makeshift mutable windows
                (0..points.len() - 1)
                    .map(|i| (i, i + 1))
                    .for_each(|(from, to)| {
                        if let [(a_pos, _a_cost), (b_pos, b_cost)] = &mut points[from..=to] {
                            let edge = self.get_edge_between(*a_pos, *b_pos)
                                .expect("edge should exist");
                            *b_cost = edge;
                        }
                    });

                Some(BlockPath::new(points))
            }
            _ => None,
        }
    }
}
