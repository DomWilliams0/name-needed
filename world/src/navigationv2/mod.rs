use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::hint::unreachable_unchecked;
use std::num::NonZeroU8;

use petgraph::graphmap::DiGraphMap;
use petgraph::stable_graph::EdgeIndex;

use misc::Itertools;
use unit::world::{
    BlockCoord, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SliceIndex,
    WorldPoint, WorldPointRange, BLOCKS_PER_METRE, BLOCKS_SCALE, CHUNK_SIZE, SLAB_SIZE,
};
pub use world_graph::{PathExistsResult, WorldArea};

use crate::chunk::slab::SliceNavArea;
use crate::chunk::slice_navmesh::{SliceAreaIndex, SliceAreaIndexAllocator};
use crate::chunk::AreaInfo;
use crate::navigationv2::world_graph::WorldGraphNodeIndex;
use crate::neighbour::NeighbourOffset;
use crate::{WorldAreaV2, ABSOLUTE_MAX_FREE_VERTICAL_SPACE};

pub mod accessible;
pub mod world_graph;

/// Area within a slab
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct SlabArea {
    pub slice_idx: LocalSliceIndex,
    pub slice_area: SliceAreaIndex,
}

/// Area within a chunk
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
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
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct SlabNavEdge {
    /// Max height in blocks for someone to pass this edge
    clearance: NonZeroU8,

    /// 0=flat, >0 step up/jump (or step down/drop if going other way)
    pub height_diff: u8,
}

#[derive(Copy, Clone)]
pub struct DirectionalSlabNavEdge<'a> {
    edge: &'a SlabNavEdge,
    edge_id: EdgeIndex,
    is_outgoing: bool,
    other_node: WorldGraphNodeIndex,
}

/// Undirected as all edges are bidirectional. Nodes are the unique slab area.
// TODO probably is immutable and recreated on any modification so rewrite to be more efficient one day
type SlabNavGraphType = DiGraphMap<SlabArea, SlabNavEdge>;

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

        debug_assert!(is_sorted_by(areas.iter(), |a| a.slice));

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
            ay1 <= by2 && ay2 >= by1
        }
        South | North => {
            // check x axis only
            ax1 <= bx2 && ax2 >= bx1
        }
        _ => unreachable!(), // cant be unaligned
    }
}

pub fn is_border(
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

pub fn as_border_area(
    area: ChunkArea,
    info: &AreaInfo,
    direction: NeighbourOffset,
) -> Option<(SliceNavArea, SliceAreaIndex)> {
    is_border(direction, info.range).then(|| {
        (
            SliceNavArea {
                slice: area.slab_area.slice_idx,
                from: info.range.0,
                to: info.range.1,
                height: info.height,
            },
            area.slab_area.slice_area,
        )
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

pub fn is_top_area(a: &SliceNavArea) -> bool {
    a.slice.slice() + a.height > SLAB_SIZE.as_u8()
}

pub fn is_bottom_area(a: &SliceNavArea) -> bool {
    a.slice == LocalSliceIndex::bottom()
}

/// Filters areas that protrude into slab above.
/// Input must be in order and unfiltered
pub fn filter_top_areas(
    areas: impl Iterator<Item = SliceNavArea>,
) -> impl Iterator<Item = (SliceNavArea, SliceAreaIndex)> {
    const MIN_SLICE: u8 = SLAB_SIZE.as_u8() - 1 - ABSOLUTE_MAX_FREE_VERTICAL_SPACE;
    let mut alloc = SliceAreaIndexAllocator::default();
    areas
        .skip_while(|a| a.slice.slice() < MIN_SLICE)
        .filter_map(move |a| {
            let idx = alloc.allocate(a.slice.slice());
            is_top_area(&a).then_some((a, idx))
        })
}

/// Filters areas that
/// Input must be in order and unfiltered
pub fn filter_bottom_areas(
    areas: impl Iterator<Item = SliceNavArea>,
) -> impl Iterator<Item = (SliceNavArea, SliceAreaIndex)> {
    const MIN_SLICE: u8 = SLAB_SIZE.as_u8() - 1 - ABSOLUTE_MAX_FREE_VERTICAL_SPACE;
    let mut alloc = SliceAreaIndexAllocator::default();
    areas
        .skip_while(|a| a.slice.slice() < MIN_SLICE)
        .filter_map(move |a| {
            let idx = alloc.allocate(a.slice.slice());
            (a.slice.slice() + a.height >= SLAB_SIZE.as_u8()).then_some((a, idx))
        })
}

fn is_sorted_by<'a, T: 'a, P: PartialOrd>(
    iter: impl Iterator<Item = &'a T>,
    key: impl Fn(&T) -> P,
) -> bool {
    iter.tuple_windows().all(|(a, b)| key(a) <= key(b))
}

fn no_dupes(input: &[(SliceNavArea, SliceAreaIndex)]) -> bool {
    let orig = input.iter().collect_vec();
    let dedup = orig
        .iter()
        .map(|(a, i)| (a.slice, i.0))
        .collect::<HashSet<_>>();
    orig.len() == dedup.len()
}

/// Edge direction is from this to the other slab, or None if directly above
pub fn discover_border_edges(
    this_areas: &[(SliceNavArea, SliceAreaIndex)],
    neighbour_areas: &[(SliceNavArea, SliceAreaIndex)],
    neighbour_dir: Option<NeighbourOffset>,
    mut on_edge: impl FnMut(SlabArea, SlabArea, SlabNavEdge),
) {
    // all areas are touching the border, but not necessarily each other
    debug_assert!(neighbour_dir.map(|d| d.is_aligned()).unwrap_or(true));
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

        /// Effective start slice in neighbour slab reference
        start_slice: u8,
    }

    // all this_slab should protrude if vertical
    if neighbour_dir.is_none() {
        for (a, _) in this_areas {
            debug_assert!(
                a.slice.slice() + a.height >= SLAB_SIZE.as_u8(),
                "this area {a:?} does not protrude into slab above"
            );
        }
    }

    impl AreaInProgress {
        fn new(
            slice_area: SliceAreaIndex,
            area: &SliceNavArea,
            this_slab: bool,
            vertical: bool,
        ) -> Self {
            let (start_slice, height_left) = if this_slab && vertical {
                (0, (area.slice.slice() + area.height) % SLAB_SIZE.as_u8())
            } else {
                (area.slice.slice(), area.height)
            };

            debug_assert_ne!(
                height_left, 0,
                "bad area (start={start_slice}, height={height_left}) {area:?}"
            );
            Self {
                this_slab,
                height_left,
                range: (area.from, area.to),
                area: SlabArea {
                    slice_idx: area.slice,
                    slice_area,
                },
                start_slice,
            }
        }
    }

    let mut working = Vec::<AreaInProgress>::with_capacity(32);
    let areas_by_slice = {
        let vertical = neighbour_dir.is_none();
        let this = this_areas
            .iter()
            .map(|(a, i)| AreaInProgress::new(*i, a, true, vertical));

        let neighbours = neighbour_areas
            .iter()
            .map(|(a, i)| AreaInProgress::new(*i, a, false, vertical));

        this.chain(neighbours) // should keep it mostly sorted
            .sorted_unstable_by_key(|a| a.start_slice)
            .group_by(|a| a.start_slice)
    };

    let mut last_slice_idx = 0;
    for (slice, areas) in &areas_by_slice {
        // decay prev slice (nonsense on first iter but working is empty)
        let decay = slice - last_slice_idx;
        last_slice_idx = slice;
        working.retain_mut(|a| {
            a.height_left = a.height_left.saturating_sub(decay);
            a.height_left > 0
        });

        // apply new areas
        working.extend(areas);

        // link up
        debug_assert!(working.iter().all(|a| a.height_left > 0));

        for (i, a) in working.iter().enumerate() {
            // skip all considered so far and this one itself, and any out of reach
            for b in working
                .iter()
                .skip(i + 1)
                .filter(|x| x.this_slab != a.this_slab && x.start_slice == slice)
            {
                let touching = match neighbour_dir {
                    Some(dir) => border_areas_touch(a.range, b.range, dir),
                    None => areas_touch(a.range, b.range),
                };
                if touching {
                    debug_assert!(
                        a.start_slice <= b.start_slice,
                        "edge should be upwards only"
                    );
                    let clearance = a.height_left.min(b.height_left);

                    unsafe {
                        let diff = if neighbour_dir.is_none() {
                            // vertical
                            (b.start_slice + SLAB_SIZE.as_u8()) - a.area.slice_idx.slice() + 1
                        } else {
                            // horizontal
                            debug_assert!(
                                a.area.slice_idx.slice() <= b.area.slice_idx.slice(),
                                "{a:?} < {b:?}"
                            );

                            b.area
                                .slice_idx
                                .slice()
                                .checked_sub(a.area.slice_idx.slice())
                                .unwrap_or_else(|| unreachable_unchecked()) // a <= b
                        };

                        // src must be from this slab to neighbour
                        // TODO also reverse edge height diff?
                        let (src, dst) = if a.this_slab {
                            (a.area, b.area)
                        } else {
                            (b.area, a.area)
                        };

                        on_edge(
                            src,
                            dst,
                            SlabNavEdge {
                                clearance: NonZeroU8::new_unchecked(clearance), // all zeroes purged already
                                height_diff: diff,
                            },
                        );
                    }
                }
            }
        }
    }
}

// pub fn discover_new_bottom_slice_areas_from_below()

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

impl SlabArea {
    pub fn to_chunk_area(self, slab: SlabIndex) -> ChunkArea {
        ChunkArea {
            slab_idx: slab,
            slab_area: self,
        }
    }
}

impl ChunkArea {
    pub fn to_world_area(self, chunk: ChunkLocation) -> WorldAreaV2 {
        WorldAreaV2 {
            chunk_idx: chunk,
            chunk_area: self,
        }
    }

    pub fn slice(self) -> GlobalSliceIndex {
        self.slab_area.slice_idx.to_global(self.slab_idx)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct NavRequirement {
    pub height: u8,
    pub step_size: u8,
    /// Blocks
    pub dims: (f32, f32),
    // TODO drop tolerance
}

impl NavRequirement {
    pub const MIN: Self = Self {
        height: 1,
        dims: (1.0, 1.0),
        step_size: 1,
    };
    pub const MAX_HEIGHT: Self = Self {
        height: ABSOLUTE_MAX_FREE_VERTICAL_SPACE,
        dims: (1.0, 1.0),
        step_size: 1,
    };
    pub const ZERO: Self = Self {
        height: 0,
        dims: (1.0, 1.0),
        step_size: 1,
    };

    pub fn with_height(height: u8) -> Self {
        Self {
            height,
            dims: (1.0, 1.0),
            step_size: 1,
        }
    }

    /// Input is in blocks!
    pub fn new(block_size: (f32, f32, f32), step_size_meters: f32) -> Self {
        debug_assert!(step_size_meters.is_sign_positive());
        Self {
            height: (block_size.2.ceil() as u8).min(ABSOLUTE_MAX_FREE_VERTICAL_SPACE),
            dims: (block_size.0, block_size.1),
            step_size: (step_size_meters * BLOCKS_PER_METRE.as_f32()) as u8,
        }
    }

    /// In blocks
    pub fn xy_diagonal_sqrd(&self) -> f32 {
        (self.dims.0 * self.dims.0) + (self.dims.1 * self.dims.1)
    }

    /// In blocks around given centre
    pub fn max_rotated_aabb(&self, centre: WorldPoint) -> WorldPointRange {
        let (w, h) = self.dims;
        let hw = w * 0.5;
        let hh = h * 0.5;
        let diag = ((hw * hw) + (hh * hh)).sqrt();
        WorldPointRange::with_inclusive_range(
            centre + (-diag, -diag, 0.0),
            centre + (diag, diag, self.height as f32 * BLOCKS_SCALE),
        )
    }
}

impl DirectionalSlabNavEdge<'_> {
    pub fn clearance(&self) -> u8 {
        self.edge.clearance.get()
    }

    pub fn step(&self) -> i8 {
        let h = self.edge.height_diff as i8;
        if self.is_outgoing {
            h
        } else {
            -h
        }
    }

    pub fn other_node(&self) -> WorldGraphNodeIndex {
        self.other_node
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use misc::Itertools;

    use super::*;

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
        dir: Option<NeighbourOffset>,
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
            Some(NeighbourOffset::East),
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
            Some(NeighbourOffset::East),
        );

        assert_eq!(edges, vec![])
    }

    #[test]
    fn neighbours_vertical_protrude() {
        let edges = do_it_neighbours(
            // below slab protrudes a few blocks into above
            vec![TestArea {
                slice: SLAB_SIZE.as_u8() - 3,
                height: 5,
                bounds: ((5, 5), (5, 5)),
            }],
            vec![TestArea {
                slice: 0,
                height: 4,
                bounds: ((6, 5), (6, 5)),
            }],
            None, // above
        );

        assert_eq!(
            edges,
            vec![(
                SlabArea {
                    // below slab
                    slice_idx: LocalSliceIndex::new_unchecked(SLAB_SIZE.as_u8() - 3),
                    slice_area: SliceAreaIndex(0),
                },
                SlabArea {
                    // above slab
                    slice_idx: LocalSliceIndex::new_unchecked(0),
                    slice_area: SliceAreaIndex(0),
                },
                SlabNavEdge {
                    clearance: NonZeroU8::new(2).unwrap(),
                    // 29,30,31|32,33
                    // X        X
                    height_diff: 4, // 29 -> 32
                }
            )]
        );
    }

    #[test]
    fn top_area_check() {
        let area = |slice: u8, height| SliceNavArea {
            slice: LocalSliceIndex::new(slice).unwrap(),
            from: (0, 0),
            to: (1, 1),
            height,
        };

        let base = SLAB_SIZE.as_u8() - 3;
        assert!(is_top_area(&area(base, 5)));
        assert!(is_top_area(&area(base, 4)));
        assert!(!is_top_area(&area(base, 3))); // touches top only but no protrusion
        assert!(!is_top_area(&area(base, 2)));
        assert!(!is_top_area(&area(base, 1)));
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

    #[test]
    fn no_diagonal_border_edges() {
        // diagonal to each other
        let a = ((0, 0), (0, 0));
        let b = ((1, 15), (1, 15));
        let a_to_b = NeighbourOffset::South;

        assert!(!border_areas_touch(a, b, a_to_b));
        assert!(!border_areas_touch(b, a, a_to_b.opposite()));
    }
}
