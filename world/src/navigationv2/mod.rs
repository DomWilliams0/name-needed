use std::hint::unreachable_unchecked;
use std::num::NonZeroU8;

use misc::{debug, Itertools};
use petgraph::graphmap::UnGraphMap;
use petgraph::prelude::DiGraphMap;
use unit::world::{BlockCoord, LocalSliceIndex, SlabIndex, SliceBlock, SliceIndex, CHUNK_SIZE};

use crate::chunk::slab::SliceNavArea;
use crate::chunk::slice_navmesh::SliceAreaIndex;
use crate::{flatten_coords, SLICE_SIZE};

/// Area within a slab
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct SlabArea {
    pub slice_idx: LocalSliceIndex,
    pub slice_area: SliceAreaIndex,
}

/// Area within a chunk
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ChunkArea {
    pub slab_idx: SlabIndex,
    pub slab_area: SlabArea,
}

/// Graph of areas within a single slab
#[derive(Clone)]
pub struct SlabNavGraph {
    graph: SlabNavGraphType,
}

// TODO make these 2xu4 instead
#[derive(Copy, Clone)]
#[cfg_attr(test, derive(Debug, Eq, PartialEq))]
pub struct SlabNavEdge {
    /// Max height in blocks for someone to pass this edge
    clearance: NonZeroU8,

    /// 0=flat, >0 step up/jump (or step down/drop if going other way)
    pub height_diff: u8,
}

/// Undirected as all edges are bidirectional. Nodes are the unique slab area.
// TODO probably is immutable and recreated on any modification so rewrite to be more efficient one day
type SlabNavGraphType = UnGraphMap<SlabArea, SlabNavEdge>;

impl SlabNavGraph {
    pub fn empty() -> Self {
        Self {
            graph: Default::default(),
        }
    }

    pub fn discover(areas: &[SliceNavArea]) -> Self {
        if areas.is_empty() {
            return Self::empty();
        }

        #[derive(Debug)]
        struct AreaInProgress {
            height_left: u8,
            area: SlabArea,
            /// (from x, from y, to x, to y) inclusive
            range: ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
        }

        let mut working = Vec::<AreaInProgress>::with_capacity(32);
        let areas_by_slice_iter = areas.iter().group_by(|a| a.slice);
        let areas_by_slice = areas_by_slice_iter.into_iter();

        debug_assert_eq!(
            areas,
            areas
                .iter()
                .copied()
                .sorted_by_key(|a| a.slice)
                .collect_vec()
                .as_slice()
        );

        let mut graph = SlabNavGraphType::with_capacity(32, 16);

        for (_, areas) in areas_by_slice {
            // decay prev slice
            working.retain_mut(|a| {
                a.height_left = a.height_left.saturating_sub(1);
                a.height_left > 0
            });

            // apply new areas
            working.extend(areas.enumerate().map(|(i, a)| {
                let area = SlabArea {
                    slice_idx: a.slice,
                    slice_area: SliceAreaIndex(i as u8),
                };

                // also create node at the same time
                graph.add_node(area);

                AreaInProgress {
                    height_left: a.height,
                    area,
                    range: (a.from, a.to),
                }
            }));

            // link up
            debug_assert!(working.iter().all(|a| a.height_left > 0));

            for (i, a) in working.iter().enumerate() {
                // skip all considered so far and this one itself, and any out of reach
                let max_slice = a.area.slice_idx.slice() + a.height_left;
                for b in working
                    .iter()
                    .skip(i + 1)
                    .take_while(|x| x.area.slice_idx.slice() <= max_slice)
                {
                    debug_assert_ne!(a.area, b.area);

                    if areas_touch(a.range, b.range) && !graph.contains_edge(a.area, b.area) {
                        debug_assert!(
                            a.area.slice_idx <= b.area.slice_idx,
                            "edge should be upwards only"
                        );
                        let clearance = a.height_left.min(b.height_left);

                        let prev = graph.add_edge(a.area, b.area, unsafe {
                            SlabNavEdge {
                                clearance: NonZeroU8::new_unchecked(clearance), // all zeroes purged already
                                height_diff: b
                                    .area
                                    .slice_idx
                                    .slice()
                                    .checked_sub(a.area.slice_idx.slice())
                                    .unwrap_or_else(|| unreachable_unchecked()), // a <= b
                            }
                        });
                        debug_assert!(prev.is_none(), "no duplicates possible")
                    }
                }
            }
        }

        Self { graph }
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = SlabArea> + '_ {
        self.graph.nodes()
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = (SlabArea, SlabArea, &SlabNavEdge)> + '_ {
        self.graph.all_edges()
    }
}

fn areas_touch(
    ((ax1, ay1), (ax2, ay2)): ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
    ((bx1, by1), (bx2, by2)): ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
) -> bool {
    let (ax1, ay1) = (ax1.saturating_sub(1), ay1.saturating_sub(1));
    let (ax2, ay2) = (ax2 + 1, ay2 + 1);
    ax1 <= bx2 && ax2 >= bx1 && ay1 <= by2 && ay2 >= by1
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use super::*;
    use misc::Itertools;

    #[test]
    fn touching() {
        assert!(areas_touch(((1, 1), (2, 2)), ((3, 1), (4, 2)),)); // adjacent
        assert!(!areas_touch(((1, 1), (2, 2)), ((4, 1), (4, 2)),)); // 1 gap inbetween
    }

    struct TestArea {
        slice: u8,
        height: u8,
        /// Inclusive
        bounds: ((u8, u8), (u8, u8)),
    }

    #[derive(Eq, PartialEq, Debug)]
    struct TestNode {
        slice: u8,
        slice_area: u8,
    }

    #[derive(Eq, PartialEq, Debug)]
    struct TestEdge {
        from: TestNode,
        to: TestNode,
        edge: SlabNavEdge,
    }

    fn do_it(areas: Vec<TestArea>) -> SlabNavGraph {
        let areas = areas
            .into_iter()
            .map(|a| SliceNavArea {
                slice: LocalSliceIndex::new_unchecked(a.slice),
                from: a.bounds.0,
                to: a.bounds.1,
                height: a.height,
            })
            .collect_vec();

        SlabNavGraph::discover(&areas)
    }

    fn edges(graph: &SlabNavGraph) -> Vec<TestEdge> {
        graph
            .graph
            .all_edges()
            .map(|(a, b, e)| TestEdge {
                from: TestNode {
                    slice: a.slice_idx.slice(),
                    slice_area: a.slice_area.0,
                },
                to: TestNode {
                    slice: b.slice_idx.slice(),
                    slice_area: b.slice_area.0,
                },
                edge: *e,
            })
            .collect()
    }

    /*

      0 1 2 3 4
    0
    1   a a b b
    2   a a b b
    3
    */

    #[test]
    fn simple_step_up_same_roof() {
        let x = do_it(vec![
            TestArea {
                slice: 1,
                height: 3,
                bounds: ((1, 1), (2, 2)),
            },
            TestArea {
                slice: 2,
                height: 2,
                bounds: ((3, 1), (4, 2)),
            },
        ]);

        // step up with clearance of 2
        assert_eq!(x.graph.node_count(), 2);
        assert_eq!(
            edges(&x),
            vec![TestEdge {
                from: TestNode {
                    slice: 1,
                    slice_area: 0
                },
                to: TestNode {
                    slice: 2,
                    slice_area: 0
                },
                edge: SlabNavEdge {
                    clearance: NonZeroU8::new(2).unwrap(),
                    height_diff: 1
                },
            }]
        );
    }

    #[test]
    fn simple_flat_walk_same_roof() {
        let x = do_it(vec![
            TestArea {
                slice: 1,
                height: 3,
                bounds: ((1, 1), (2, 2)),
            },
            TestArea {
                slice: 1,
                height: 3,
                bounds: ((3, 1), (4, 2)),
            },
        ]);

        assert_eq!(x.graph.node_count(), 2);
        assert_eq!(
            edges(&x),
            vec![TestEdge {
                from: TestNode {
                    slice: 1,
                    slice_area: 0
                },
                to: TestNode {
                    slice: 1,
                    slice_area: 1
                },
                edge: SlabNavEdge {
                    clearance: NonZeroU8::new(3).unwrap(),
                    height_diff: 0
                },
            }]
        );
    }

    #[test]
    fn dont_link_with_inaccessible_roof() {
        let x = do_it(vec![
            TestArea {
                slice: 1,
                height: 3,
                bounds: ((1, 1), (2, 2)),
            },
            TestArea {
                slice: 5,
                height: 3,
                bounds: ((1, 1), (2, 2)),
            },
        ]);

        assert_eq!(edges(&x), vec![]);
    }

    #[test]
    fn single_area() {
        let x = do_it(vec![TestArea {
            slice: 1,
            height: 3,
            bounds: ((1, 1), (2, 2)),
        }]);

        assert_eq!(x.graph.node_count(), 1); // should still have a node
    }
}
