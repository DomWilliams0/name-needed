use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::hint::unreachable_unchecked;
use std::num::NonZeroU8;

use misc::{debug, trace, Either, Itertools};
use petgraph::graphmap::UnGraphMap;
use petgraph::prelude::DiGraphMap;
use unit::world::{
    BlockCoord, ChunkLocation, LocalSliceIndex, SlabIndex, SlabLocation, SliceBlock, SliceIndex,
    CHUNK_SIZE,
};

use crate::chunk::slab::SliceNavArea;
use crate::chunk::slice_navmesh::{SliceAreaIndex, SliceAreaIndexAllocator};
use crate::chunk::AreaInfo;
use crate::neighbour::NeighbourOffset;
use crate::{flatten_coords, SLICE_SIZE};

pub mod world_graph;

pub use world_graph::WorldArea;

/// Area within a slab
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
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
#[cfg_attr(any(test, debug_assertions), derive(Debug, Eq, PartialEq, Hash))]
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

        debug_assert!(is_sorted_by(areas, |a| a.slice));

        let mut graph = SlabNavGraphType::with_capacity(32, 16);

        let mut last_slice_idx = 0;
        for (slice, areas) in areas_by_slice {
            // decay prev slice (nonsense on first iter but working is empty)
            let decay = slice.slice() - last_slice_idx;
            last_slice_idx = slice.slice();
            working.retain_mut(|a| {
                a.height_left = a.height_left.saturating_sub(decay);
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
                for b in working
                    .iter()
                    .skip(i + 1)
                    .filter(|x| x.area.slice_idx == slice)
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

/// True for adjacent areas that are not diagonals
fn areas_touch(
    ((ax1, ay1), (ax2, ay2)): ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
    ((bx1, by1), (bx2, by2)): ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
) -> bool {
    let intersects =
        |((ax1, ay1), (ax2, ay2))| ax1 <= bx2 && ax2 >= bx1 && ay1 <= by2 && ay2 >= by1;

    // expand a outwards in a cross and check for intersection
    intersects(((ax1.saturating_sub(1), ay1), (ax2 + 1, ay2)))
        || intersects(((ax1, ay1.saturating_sub(1)), (ax2, ay2 + 1)))
}

fn border_areas_touch(
    ((ax1, ay1), (ax2, ay2)): ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
    ((bx1, by1), (bx2, by2)): ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
    dir: NeighbourOffset,
) -> bool {
    use NeighbourOffset::*;

    match dir {
        East | West => {
            // check y axis only
            let ay1 = ay1.saturating_sub(1);
            let ay2 = ay2 + 1;
            ay1 <= by2 && ay2 >= by1
        }
        South | North => {
            // check x axis only
            let ax1 = ax1.saturating_sub(1);
            let ax2 = ax2 + 1;
            ax1 <= bx2 && ax2 >= bx1
        }
        _ => unreachable!(), // cant be unaligned
    }
}

fn is_border(
    direction: NeighbourOffset,
    range: ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
) -> bool {
    use NeighbourOffset::*;
    let is_min = |coord| coord == 0;
    let is_max = |coord| coord == CHUNK_SIZE.as_block_coord() - 1;

    let (from, to) = range;
    match direction {
        South => is_min(from.1),
        East => is_max(to.0),
        North => is_max(to.1),
        West => is_min(from.0),
        _ => false, // cannot link with diagonals
    }
}

pub fn filter_border_areas_with_info(
    areas: impl Iterator<Item = (ChunkArea, AreaInfo)>,
    direction: NeighbourOffset,
) -> impl Iterator<Item = (SliceNavArea, SliceAreaIndex)> {
    areas.filter_map(move |(a, i)| {
        is_border(direction, i.range).then(|| {
            (
                SliceNavArea {
                    slice: a.slab_area.slice_idx,
                    from: i.range.0,
                    to: i.range.1,
                    height: i.height,
                },
                a.slab_area.slice_area,
            )
        })
    })
}

/// Input must be in order and unfiltered
pub fn filter_border_areas(
    areas: impl Iterator<Item = SliceNavArea>,
    direction: NeighbourOffset,
) -> impl Iterator<Item = (SliceNavArea, SliceAreaIndex)> {
    let mut alloc = SliceAreaIndexAllocator::default();
    areas.filter_map(move |a| {
        let idx = alloc.allocate(a.slice.slice());
        is_border(direction, (a.from, a.to)).then_some((a, idx))
    })
}

fn is_sorted_by<T, P: PartialOrd>(slice: &[T], key: impl Fn(&T) -> P) -> bool {
    slice.iter().tuple_windows().all(|(a, b)| key(a) <= key(b))
}

fn no_dupes(input: &[(SliceNavArea, SliceAreaIndex)]) -> bool {
    let orig = input.iter().collect_vec();
    let dedup = orig
        .iter()
        .map(|(a, i)| (a.slice, i.0))
        .collect::<HashSet<_>>();
    orig.len() == dedup.len()
}

/// Edge direction is from this to the other slab
pub fn discover_border_edges(
    this_areas: &[(SliceNavArea, SliceAreaIndex)],
    neighbour_areas: &[(SliceNavArea, SliceAreaIndex)],
    neighbour_dir: NeighbourOffset,
    mut on_edge: impl FnMut(SlabArea, SlabArea, SlabNavEdge),
) {
    // all areas are touching the border, but not necessarily each other
    debug_assert!(neighbour_dir.is_aligned());
    debug_assert!(no_dupes(this_areas));
    debug_assert!(no_dupes(neighbour_areas));

    #[derive(Debug)]
    struct AreaInProgress {
        /// False = neighbour
        this_slab: bool,

        height_left: u8,
        area: SlabArea,
        /// (from x, from y, to x, to y) inclusive within its own slab
        range: ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
    }

    impl AreaInProgress {
        fn new(slice_area: SliceAreaIndex, area: &SliceNavArea, this_slab: bool) -> Self {
            Self {
                this_slab,
                height_left: area.height,
                range: (area.from, area.to),
                area: SlabArea {
                    slice_idx: area.slice,
                    slice_area,
                },
            }
        }
    }

    let mut working = Vec::<AreaInProgress>::with_capacity(32);
    let areas_by_slice = {
        let this = this_areas
            .iter()
            .map(|(a, i)| AreaInProgress::new(*i, a, true));

        let neighbours = neighbour_areas
            .iter()
            .map(|(a, i)| AreaInProgress::new(*i, a, false));

        this.interleave(neighbours) // should keep it mostly sorted
            .sorted_unstable_by_key(|a| a.area.slice_idx)
            .group_by(|a| a.area.slice_idx)
    };

    let mut last_slice_idx = 0;
    for (slice, areas) in &areas_by_slice {
        // decay prev slice (nonsense on first iter but working is empty)
        let decay = slice.slice() - last_slice_idx;
        last_slice_idx = slice.slice();
        working.retain_mut(|a| {
            a.height_left = a.height_left.saturating_sub(decay);
            a.height_left > 0
        });

        // apply new areas
        working.extend(areas);

        // link up
        debug_assert!(working.iter().all(|a| a.height_left > 0));

        for (i, a) in working.iter().enumerate().filter(|(_, a)| a.this_slab) {
            // skip all considered so far and this one itself, and any out of reach
            for b in working
                .iter()
                .skip(i + 1)
                .filter(|x| !x.this_slab && x.area.slice_idx == slice)
            {
                if border_areas_touch(a.range, b.range, neighbour_dir) {
                    debug_assert!(
                        a.area.slice_idx <= b.area.slice_idx,
                        "edge should be upwards only"
                    );
                    let clearance = a.height_left.min(b.height_left);

                    // TODO ensure no dups
                    on_edge(a.area, b.area, unsafe {
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
                }
            }
        }
    }
}

impl Display for SlabArea {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SlabArea({}:{})", self.slice_idx, self.slice_area.0)
    }
}

impl Debug for SlabArea {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use super::*;
    use misc::Itertools;
    use std::collections::HashSet;

    #[test]
    fn touching() {
        assert!(areas_touch(((1, 1), (2, 2)), ((3, 1), (4, 2)),)); // adjacent
        assert!(!areas_touch(((1, 1), (2, 2)), ((4, 1), (4, 2)),)); // 1 gap inbetween

        assert!(!areas_touch(((1, 1), (2, 2)), ((3, 3), (4, 4)),)); // diagonal
        assert!(areas_touch(((1, 1), (2, 3)), ((3, 3), (4, 4)),)); // now actually touching
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

    fn do_it_neighbours(
        this: Vec<TestArea>,
        other: Vec<TestArea>,
        dir: NeighbourOffset,
    ) -> Vec<(SlabArea, SlabArea, SlabNavEdge)> {
        let mut alloc = SliceAreaIndexAllocator::default();
        let this = this
            .into_iter()
            .sorted_by_key(|x| x.slice)
            .map(|a| {
                (
                    SliceNavArea {
                        slice: LocalSliceIndex::new_unchecked(a.slice),
                        from: a.bounds.0,
                        to: a.bounds.1,
                        height: a.height,
                    },
                    alloc.allocate(a.slice),
                )
            })
            .collect_vec();

        alloc = SliceAreaIndexAllocator::default();
        let other = other
            .into_iter()
            .sorted_by_key(|x| x.slice)
            .map(|a| {
                (
                    SliceNavArea {
                        slice: LocalSliceIndex::new_unchecked(a.slice),
                        from: a.bounds.0,
                        to: a.bounds.1,
                        height: a.height,
                    },
                    alloc.allocate(a.slice),
                )
            })
            .collect_vec();

        let mut edges = HashSet::new();
        discover_border_edges(&this, &other, dir, |a, b, e| {
            assert!(edges.insert((a, b, e)), "duplicate {:?}->{:?}", a, b)
        });
        edges.into_iter().collect_vec()
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
    fn simple_step_up_diff_roof() {
        let x = do_it(vec![
            TestArea {
                slice: 1,
                height: 3,
                bounds: ((1, 1), (2, 2)),
            },
            TestArea {
                slice: 2,
                height: 5,
                bounds: ((3, 1), (4, 2)),
            },
        ]);

        // step up with clearance of 2 still
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
    fn simple_multiple_step_up_same_roof() {
        let x = do_it(vec![
            TestArea {
                slice: 1,
                height: 3,
                bounds: ((1, 1), (2, 2)),
            },
            TestArea {
                slice: 3,
                height: 1,
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
                    slice: 3,
                    slice_area: 0
                },
                edge: SlabNavEdge {
                    clearance: NonZeroU8::new(1).unwrap(),
                    height_diff: 2
                },
            }]
        );
    }

    #[test]
    fn simple_multiple_step_up_diff_roof() {
        let x = do_it(vec![
            TestArea {
                slice: 1,
                height: 3,
                bounds: ((1, 1), (2, 2)),
            },
            TestArea {
                slice: 3,
                height: 5,
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
                    slice: 3,
                    slice_area: 0
                },
                edge: SlabNavEdge {
                    clearance: NonZeroU8::new(1).unwrap(),
                    height_diff: 2
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

    #[test]
    fn neighbours_touching() {
        let edges = do_it_neighbours(
            vec![TestArea {
                slice: 2,
                height: 5,
                bounds: ((5, 5), (15, 8)),
            }],
            vec![TestArea {
                slice: 4,
                height: 8,
                bounds: ((0, 3), (2, 8)),
            }],
            NeighbourOffset::East,
        );

        assert_eq!(
            edges,
            vec![(
                SlabArea {
                    slice_idx: LocalSliceIndex::new_unchecked(2),
                    slice_area: SliceAreaIndex(0),
                },
                SlabArea {
                    slice_idx: LocalSliceIndex::new_unchecked(4),
                    slice_area: SliceAreaIndex(0),
                },
                SlabNavEdge {
                    clearance: NonZeroU8::new(3).unwrap(),
                    height_diff: 2,
                }
            )]
        );
    }

    #[test]
    fn neighbours_not_touching() {
        let edges = do_it_neighbours(
            vec![TestArea {
                slice: 2,
                height: 5,
                bounds: ((5, 5), (15, 8)),
            }],
            vec![TestArea {
                slice: 4,
                height: 8,
                bounds: ((0, 0), (2, 3)),
            }],
            NeighbourOffset::East,
        );

        assert_eq!(edges, vec![])
    }

    #[test]
    fn all_edges() {
        // old bug: slice 2 area should link to all 5 of the slice 1 ones
        let areas = vec![
            TestArea {
                slice: 1,
                bounds: ((0, 0), (6, 2)),
                height: 4,
            },
            TestArea {
                slice: 1,
                bounds: ((7, 0), (15, 1)),
                height: 4,
            },
            TestArea {
                slice: 1,
                bounds: ((8, 2), (15, 15)),
                height: 4,
            },
            TestArea {
                slice: 1,
                bounds: ((0, 3), (6, 15)),
                height: 4,
            },
            TestArea {
                slice: 1,
                bounds: ((7, 6), (7, 15)),
                height: 4,
            },
            TestArea {
                slice: 2,
                bounds: ((7, 2), (7, 5)),
                height: 4,
            },
        ];
        let graph = do_it(areas);
        let edges = graph
            .graph
            .edges(SlabArea {
                slice_idx: LocalSliceIndex::new_unchecked(2),
                slice_area: SliceAreaIndex(0),
            })
            .collect_vec();
        assert_eq!(edges.len(), 5, "edges: {:?}", edges);
    }
}
