use crate::{
    flatten_coords, iter_slice_xy, unflatten_index, BlockType, Slab, WorldContext, SLICE_SIZE,
};
use arbitrary_int::{u3, u4, u5};
use misc::{lazy_static, trace, Itertools, SlogDrain};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;
use unit::world::{
    BlockCoord, LocalSliceIndex, SlabPosition, SliceBlock, SliceIndex, CHUNK_SIZE, SLAB_SIZE,
};

/// Area index in a slice, all values are possible to allow for every block in a slice of 256 to
/// have a separate area
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Default)]
pub struct SliceAreaIndex(pub(crate) u8);

impl SliceAreaIndex {
    pub const DEFAULT: Self = Self(0);
}

#[derive(Default)]
pub struct SliceAreaIndexAllocator {
    slice: Option<u8>,
    current: u8,
}

impl SliceAreaIndexAllocator {
    pub fn allocate(&mut self, slice: u8) -> SliceAreaIndex {
        if self.slice == Some(slice) {
            self.current += 1;
        } else {
            if let Some(prev) = self.slice {
                debug_assert!(slice > prev);
            }
            self.slice = Some(slice);
            self.current = 0;
        };

        SliceAreaIndex(self.current)
    }
}

#[test]
fn slice_area_allocator() {
    let mut a = SliceAreaIndexAllocator::default();

    assert_eq!(a.allocate(2), SliceAreaIndex(0));
    assert_eq!(a.allocate(2), SliceAreaIndex(1));
    assert_eq!(a.allocate(2), SliceAreaIndex(2));
    assert_eq!(a.allocate(3), SliceAreaIndex(0));
    assert_eq!(a.allocate(3), SliceAreaIndex(1));
    assert_eq!(a.allocate(3), SliceAreaIndex(2));
    assert_eq!(a.allocate(5), SliceAreaIndex(0));
    assert_eq!(a.allocate(5), SliceAreaIndex(1));
}

pub trait SliceConfig {
    /// Len of 1 edge in square
    const SZ: usize;
    const N: usize = Self::SZ.pow(2);
    const FILL_OUTPUT: bool;
    type InputElem: Copy;

    fn available_height(elem: Self::InputElem) -> u8;

    fn emit(&mut self, range: [usize; 2], height: u8, area: u8);
}

/// Initialised array is mask of blocks with an area in output
pub fn make_mesh<C: SliceConfig>(
    cfg: &mut C,
    input: &[C::InputElem],
    output: &mut [SliceAreaIndex],
    initialised: &mut [bool],
) {
    // TODO maybe better to copy slice into a new array of the heights then just use that

    assert_eq!(output.len(), C::N); //  TODO ensure bounds checks are avoided
    assert_eq!(input.len(), C::N);
    assert_eq!(initialised.len(), C::N);
    assert_eq!(SLICE_SIZE, u8::MAX as usize + 1); // must be big enough

    let unflatten_idx = |i: usize| [i % C::SZ, i / C::SZ];
    let flatten_pos = |[x, y]: [usize; 2]| {
        debug_assert!(x < C::SZ, "x is {x}");
        debug_assert!(y < C::SZ, "y is {y}");
        (y * C::SZ) + x
    };

    let mut next_area = 0;
    let mut next_start_index = 0;

    loop {
        // find new starting corner
        let (cur_corner_idx, cur_corner_height, [cur_corner_x, cur_corner_y]) = {
            let start_idx = match initialised
                .iter()
                .skip(next_start_index)
                .position(|b| !(*b))
            {
                Some(i) => i + next_start_index,
                None => break,
            };
            trace!("finding new starting corner from {start_idx}");
            next_start_index = start_idx;
            match input
                .iter()
                .skip(start_idx)
                .enumerate()
                .find_map(|(i, elem)| {
                    let idx = start_idx + i;
                    if initialised[idx] {
                        return None;
                    }

                    let h = C::available_height(*elem);
                    (h > 0).then_some((idx, h))
                }) {
                Some((i, h)) => (i, h, unflatten_idx(i)),
                None => break, // done
            }
        };
        trace!("corner is {cur_corner_x},{cur_corner_y} idx={cur_corner_idx} height={cur_corner_height}");

        // expand in 1 direction - TODO look in both directions, choose longest
        let sz_x = input
            .iter()
            .skip(cur_corner_idx + 1)
            .take(C::SZ - cur_corner_x - 1)
            .enumerate()
            .take_while(|(rel_i, elem)| {
                !initialised[*rel_i + cur_corner_idx + 1]
                    && C::available_height(**elem) == cur_corner_height
            })
            .count()
            + 1; // dont include the starting one
        trace!("sz in x is {sz_x}");
        debug_assert!(sz_x <= C::SZ, "sz is {sz_x}");

        // expand in 2nd direction, skip current row
        trace!("checking {} rows", C::SZ - cur_corner_y);
        let mut end_corner = None;
        for expansion_row_offset in 1..=(C::SZ - cur_corner_y) {
            // check all in row
            let enumerate_offset = cur_corner_idx + (expansion_row_offset * C::SZ);
            trace!("row offset {enumerate_offset}");
            if let Some(fail_idx) = input
                .iter()
                .skip(enumerate_offset)
                .take(sz_x)
                .enumerate()
                .find_map(|(rel_i, e)| {
                    let real_idx = rel_i + enumerate_offset;
                    if initialised[real_idx] {
                        return Some(real_idx);
                    }

                    let h = C::available_height(*e);
                    (h != cur_corner_height).then_some(rel_i)
                })
            {
                end_corner = Some([cur_corner_x + fail_idx, cur_corner_y + expansion_row_offset]);
                trace!(
                    "expansion in y ended at idx {fail_idx}, end corner exclusive = {:?}",
                    end_corner
                );
                break;
            }
        }

        enum NextIteration {
            StartAt([usize; 2], u8),
            EndReached,
        }

        // TODO it can be be better to consume the previous rows excluding this one instead
        let end_corner_inclusive = match end_corner {
            None => [cur_corner_x + sz_x - 1, C::SZ - 1],
            Some([x, y]) => {
                if x == cur_corner_x {
                    // ended in previous row
                    [cur_corner_x + sz_x - 1, y - 1]
                } else {
                    // ended halfway through row
                    [x - 1, y]
                }
            }
        };

        // emit rect
        cfg.emit(
            [
                flatten_pos([cur_corner_x, cur_corner_y]),
                flatten_pos(end_corner_inclusive),
            ],
            cur_corner_height,
            next_area,
        );
        trace!(
            "emit rectangle {:?} -> {:?} area={}",
            [cur_corner_x, cur_corner_y],
            end_corner_inclusive,
            next_area
        );

        let truncated_sz_x = end_corner_inclusive[0] - cur_corner_x + 1;
        for row in cur_corner_y..=end_corner_inclusive[1] {
            let start = (row * C::SZ) + cur_corner_x;
            initialised[start..start + truncated_sz_x].fill(true);
            if C::FILL_OUTPUT {
                output[start..start + truncated_sz_x].fill(SliceAreaIndex(next_area));
            }
        }

        #[cfg(test)]
        {
            if C::SZ == 5 {
                tests::print_grid(output, initialised);
            }
        }

        next_area += 1;

        // if end_corner.is_none() {
        //     // finished
        //     trace!("reached bottom corner");
        //     break;
        // }

        // prepare for next iteration
        // let next_start_x = cur_corner_x + sz_x;
        // let next_start = if next_start_x < C::SZ {
        //     [next_start_x, cur_corner_y]
        // } else {
        //     // next row
        //     [0, cur_corner_y + 1]
        // };
        // let next_start = match initialised.iter().skip(next_start_index).position(|b| !(*b)) {
        //     Some(i) => i + next_start_index,
        //     None => break,
        // };
        //
        // next = StartAt::SearchFrom(next_start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use misc::Itertools;

    #[derive(Debug, Eq, PartialEq)]
    struct Rect {
        from: [usize; 2],
        to: [usize; 2],
        height: u8,
        area: u8,
    }

    #[derive(Default)]
    struct Cfg(Vec<Rect>);
    impl SliceConfig for Cfg {
        const SZ: usize = 5;
        const FILL_OUTPUT: bool = true;
        type InputElem = i32;

        fn available_height(elem: Self::InputElem) -> u8 {
            elem as u8
        }

        fn emit(&mut self, range: [usize; 2], height: u8, area: u8) {
            let unflatten_idx =
                |i: usize| [i % <Cfg as SliceConfig>::SZ, i / <Cfg as SliceConfig>::SZ];
            self.0.push(Rect {
                from: unflatten_idx(range[0]),
                to: unflatten_idx(range[1]),
                height,
                area,
            })
        }
    }

    pub fn print_grid(output: &[SliceAreaIndex], visited: &[bool]) {
        for row in &output.iter().zip(visited.iter()).chunks(5) {
            eprintln!(
                "{}",
                row.map(|(area, init)| if *init { (area.0 + b'0') as char } else { '.' })
                    .format(" ")
            )
        }

        eprintln!("--------");

        for row in &visited.iter().chunks(5) {
            eprintln!(
                "{}",
                row.map(|init| if *init { 'O' } else { '.' }).format(" ")
            )
        }

        eprintln!("========");
    }

    // TODO full slice, 1,1 to end, full except single block stopping

    fn do_it(input: [i32; 25]) -> (Vec<Rect>, Vec<Option<SliceAreaIndex>>) {
        let mut output = vec![SliceAreaIndex::DEFAULT; 25];
        let mut visited = [false; 25];
        let mut cfg = Cfg::default();
        make_mesh::<Cfg>(&mut cfg, &input, &mut output, &mut visited);
        eprintln!("{:?}", cfg.0);
        for (i, r) in cfg.0.iter().enumerate() {
            eprintln!("* {i}: {r:?}");
        }

        print_grid(&output, &visited);

        let to_skip = input.iter().filter(|i| **i == 0).count();
        let not_visited = visited.iter().filter(|b| !**b).count();
        assert_eq!(to_skip, not_visited);

        (
            cfg.0,
            output
                .into_iter()
                .zip(visited.into_iter())
                .map(|(val, b)| b.then_some(val))
                .collect_vec(),
        )
    }

    #[test]
    fn single() {
        let input = [
            0, 0, 0, 0, 0, //
            0, 2, 2, 2, 0, //
            0, 2, 2, 2, 0, //
            0, 2, 2, 2, 0, //
            0, 0, 0, 0, 0, //
        ];
        let (rects, out) = do_it(input);
        assert_eq!(
            rects,
            vec![Rect {
                from: [1, 1],
                to: [3, 3],
                height: 2,
                area: 0
            }]
        );
    }

    #[test]
    fn single_fully_covered() {
        let input = [
            2, 2, 2, 2, 2, //
            2, 2, 2, 2, 2, //
            2, 2, 2, 2, 2, //
            2, 2, 2, 2, 2, //
            2, 2, 2, 2, 2, //
        ];
        let (rects, out) = do_it(input);
        assert_eq!(
            rects,
            vec![Rect {
                from: [0, 0],
                to: [4, 4],
                height: 2,
                area: 0
            }]
        );
    }

    #[test]
    fn single_mostly_fully_covered() {
        let input = [
            0, 2, 2, 2, 2, //
            0, 2, 2, 2, 2, //
            0, 2, 2, 2, 2, //
            0, 2, 2, 2, 2, //
            0, 2, 2, 2, 2, //
        ];
        let (rects, out) = do_it(input);
        assert_eq!(
            rects,
            vec![Rect {
                from: [1, 0],
                to: [4, 4],
                height: 2,
                area: 0
            }]
        );
    }

    #[test]
    fn single_full_interrupted_by_single_block() {
        let input = [
            2, 2, 2, 2, 2, //
            2, 2, 2, 2, 2, //
            2, 2, 2, 0, 2, //
            2, 2, 2, 2, 2, //
            2, 2, 2, 2, 2, //
        ];
        let (rects, out) = do_it(input);
        assert_eq!(rects.len(), 4);

        assert!(out[13].is_none());
        assert_eq!(out.iter().filter(|o| o.is_some()).count(), 25 - 1);
    }

    #[test]
    fn complex() {
        let input = [
            0, 0, 0, 0, 0, //
            0, 2, 2, 4, 4, //
            0, 2, 2, 3, 3, //
            0, 2, 2, 3, 3, //
            0, 0, 0, 4, 2, //
        ];
        let (rects, out) = do_it(input);
        assert_eq!(rects.len(), 5);
        assert_ne!(out[7].unwrap(), out[8].unwrap());
        assert_ne!(out[23].unwrap(), out[24].unwrap());
    }
}

pub type FreeVerticalSpace = u8;

/// Must fit into 3 bits
pub const ABSOLUTE_MAX_FREE_VERTICAL_SPACE: FreeVerticalSpace = 8;

#[bitbybit::bitfield(u16, default: 0)]
#[cfg_attr(test, derive(PartialEq))]
struct PackedSlabBlockVerticalSpace {
    #[bits(12..=15, rw)]
    x: u4,

    #[bits(8..=11, rw)]
    y: u4,

    #[bits(3..=7, rw)]
    z: u5,

    /// Cannot be 0, so is stored as -1 so it represents 1-8
    #[bits(0..=2, rw)]
    height_but_offset: u3,
}

impl PackedSlabBlockVerticalSpace {
    fn pos(&self) -> SlabPosition {
        SlabPosition::new_unchecked(self.x().value(), self.y().value(), self.pos_z())
    }

    fn pos_z(&self) -> LocalSliceIndex {
        LocalSliceIndex::new_unchecked(self.z().value() as i32)
    }

    fn height(&self) -> FreeVerticalSpace {
        self.height_but_offset().value() + 1
    }

    fn new_(x: BlockCoord, y: BlockCoord, slice: u8, height: FreeVerticalSpace) -> Self {
        debug_assert!(height > 0);
        debug_assert!(
            height <= ABSOLUTE_MAX_FREE_VERTICAL_SPACE,
            "height {height}"
        );
        unsafe {
            PackedSlabBlockVerticalSpace::new()
                .with_x(u4::new_unchecked(x))
                .with_y(u4::new_unchecked(y))
                .with_z(u5::new_unchecked(slice))
                .with_height_but_offset(u3::new_unchecked(height.saturating_sub(1)))
        }
    }
}

impl Debug for PackedSlabBlockVerticalSpace {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("PackedSlabBlockVerticalSpace")
            .field("pos", &self.pos())
            .field("height", &self.height())
            .finish()
    }
}

#[cfg_attr(test, derive(PartialEq))]
pub struct SlabVerticalSpace {
    /// Sorted by pos ascending.
    /// pos: 16x16x32 = 4+4+5 bits = 13 bits per pos alone.
    /// height must fit into 3 bits, so up to 8m supported.
    blocks: Box<[PackedSlabBlockVerticalSpace]>,

    // TODO can be packed more e.g. RLE
    top_down: [FreeVerticalSpace; SLICE_SIZE],

    // TODO just used to check if solid, should be packed into a bit array or box<[packed xy]>
    bottom_solids: Box<[SliceBlock]>,
}

lazy_static! {
    static ref EMPTY_SLAB_VERTICAL_SPACE: Arc<SlabVerticalSpace> = SlabVerticalSpace::calc_empty();
}

impl SlabVerticalSpace {
    /// Returns shared reference
    pub fn empty() -> Arc<Self> {
        EMPTY_SLAB_VERTICAL_SPACE.clone()
    }

    fn calc_empty() -> Arc<Self> {
        let blocks = iter_slice_xy()
            .map(|b| {
                PackedSlabBlockVerticalSpace::new_(
                    b.x(),
                    b.y(),
                    0,
                    ABSOLUTE_MAX_FREE_VERTICAL_SPACE,
                )
            })
            .collect();

        Arc::new(Self {
            blocks,
            top_down: [ABSOLUTE_MAX_FREE_VERTICAL_SPACE; SLICE_SIZE],
            bottom_solids: Box::new([]),
        })
    }

    pub fn discover<C: WorldContext>(terrain: &Slab<C>) -> Arc<Self> {
        let mut out = vec![];
        let mut bottom_solids = vec![];

        #[derive(Copy, Clone, Default)]
        struct BlockState {
            first_air_seen_slice: u8,
            height_so_far: FreeVerticalSpace,
        }

        let mut working = [BlockState::default(); SLICE_SIZE];

        for (slice_idx, slice) in terrain.slices_from_bottom() {
            for (i, (b, state)) in slice.iter().zip(working.iter_mut()).enumerate() {
                if b.block_type().is_air() {
                    if state.height_so_far == 0 {
                        state.first_air_seen_slice = slice_idx.slice_unsigned() as u8;
                    }

                    state.height_so_far += 1;
                } else {
                    if state.height_so_far > 0 {
                        let (x, y) = unflatten_index(i).xy();
                        out.push(PackedSlabBlockVerticalSpace::new_(
                            x,
                            y,
                            state.first_air_seen_slice,
                            state.height_so_far.min(ABSOLUTE_MAX_FREE_VERTICAL_SPACE),
                        ));
                    }

                    *state = BlockState::default();

                    // gather bottom slice solid blocks
                    if slice_idx == LocalSliceIndex::bottom() {
                        bottom_solids.push(unflatten_index(i));
                    }
                }
            }
        }

        // use end working state for bottom up
        out.extend(working.iter().enumerate().filter_map(|(i, state)| {
            if state.height_so_far > 0 {
                let (x, y) = unflatten_index(i).xy();
                Some(PackedSlabBlockVerticalSpace::new_(
                    x,
                    y,
                    state.first_air_seen_slice,
                    state.height_so_far.min(ABSOLUTE_MAX_FREE_VERTICAL_SPACE),
                ))
            } else {
                None
            }
        }));

        // also use for top down
        let mut top_down = [0; SLICE_SIZE];
        for (state, out) in working.iter().zip(top_down.iter_mut()) {
            *out = state.height_so_far.min(ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
        }

        out.sort_unstable_by_key(|b| b.pos());
        Arc::new(Self {
            blocks: out.into_boxed_slice(),
            top_down,
            bottom_solids: bottom_solids.into_boxed_slice(),
        })
    }

    /// Sorted asc by slice
    pub fn iter_blocks(&self) -> impl Iterator<Item = (SlabPosition, FreeVerticalSpace)> + '_ {
        self.blocks.iter().map(|i| (i.pos(), i.height()))
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn above_at(&self, (x, y): (BlockCoord, BlockCoord)) -> FreeVerticalSpace {
        self.top_down[flatten_coords(SliceBlock::new_unchecked(x, y))]
    }

    pub fn above(&self) -> &[FreeVerticalSpace; SLICE_SIZE] {
        &self.top_down
    }

    pub fn iter_above(&self) -> impl Iterator<Item = (SliceBlock, FreeVerticalSpace)> + '_ {
        (0..CHUNK_SIZE.as_block_coord())
            .cartesian_product(0..CHUNK_SIZE.as_block_coord())
            .zip(self.top_down.into_iter())
            .map(|((y, x), h)| (SliceBlock::new_srsly_unchecked(x, y), h))
    }

    pub fn below_at(&self, (x, y): (BlockCoord, BlockCoord)) -> FreeVerticalSpace {
        match self
            .blocks
            .iter()
            .take_while(|p| p.pos_z() == LocalSliceIndex::bottom())
            .find(|i| i.x().value() == x && i.y().value() == y)
        {
            Some(b) => b.height(),
            None if self
                .bottom_solids
                .contains(&SliceBlock::new_srsly_unchecked(x, y)) =>
            {
                0 // solid
            }
            None => ABSOLUTE_MAX_FREE_VERTICAL_SPACE, // air
        }
    }

    /// Only returns exact matches for the bottom of the vertical space
    pub fn find_block_exact(&self, pos: SlabPosition) -> Option<FreeVerticalSpace> {
        self.blocks
            .binary_search_by_key(&pos, |x| x.pos())
            .ok()
            .map(|i| self.blocks[i].height())
    }

    /// Searches downward for a block with vertical space above it
    pub fn find_slice(&self, pos: SlabPosition) -> Option<LocalSliceIndex> {
        let tgt_slice = pos.z().slice();
        let min = LocalSliceIndex::bottom().slice_unsigned() as u8; // tgt_slice.saturating_sub(ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
        for z in (min..=tgt_slice).rev() {
            let b =
                SlabPosition::new_unchecked(pos.x(), pos.y(), LocalSliceIndex::new_unchecked(z));
            match self.blocks.binary_search_by_key(&b, |x| x.pos()) {
                Err(_) => continue,
                Ok(idx) => {
                    let h = &self.blocks[idx];
                    let candidate_z = h.pos_z();

                    let candidate_max = candidate_z.slice() + h.height();
                    return (tgt_slice < candidate_max).then_some(candidate_z);
                }
            }
        }

        None
    }
}

impl Debug for SlabVerticalSpace {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("SlabVerticalSpace")
            .field("blocks", &self.blocks.len())
            .field("top_down", &self.top_down.iter().counts())
            .finish()
    }
}

#[cfg(test)]
mod tests_vertical_space {
    use super::*;
    use crate::helpers::{
        load_single_chunk, loader_from_chunks_blocking,
        loader_from_chunks_blocking_with_load_blacklist, world_from_chunks_blocking,
        DummyBlockType, DummyWorldContext,
    };
    use crate::loader::WorldTerrainUpdate;
    use crate::navigationv2::{NavRequirement, SlabNavEdge};
    use crate::ChunkBuilder;
    use std::iter::once;
    use std::thread::sleep;
    use std::time::Duration;
    use unit::world::{
        BlockPosition, ChunkLocation, GlobalSliceIndex, SlabIndex, SlabLocation, WorldPosition,
        WorldPositionRange, CHUNK_SIZE,
    };

    #[test]
    fn packed() {
        let x = PackedSlabBlockVerticalSpace::new_(5, 6, 7, ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
        assert_eq!(
            x.pos(),
            SlabPosition::new_unchecked(5, 6, LocalSliceIndex::new_unchecked(7))
        );
        assert_eq!(x.height(), ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
    }

    #[test]
    fn floating_blocks() {
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .set_block((3, 3, 3), DummyWorldContext::PRESET_TYPES[0])
            .set_block((3, 3, 7), DummyWorldContext::PRESET_TYPES[0])
            .set_block(
                (3, 3, SLAB_SIZE.as_i32() - 3),
                DummyWorldContext::PRESET_TYPES[0],
            )
            // ceiling
            .set_block(
                (4, 4, SLAB_SIZE.as_i32() - 1),
                DummyWorldContext::PRESET_TYPES[0],
            )
            // pillar
            .set_block((5, 5, 20), DummyWorldContext::PRESET_TYPES[0])
            .set_block((5, 5, 21), DummyWorldContext::PRESET_TYPES[0])
            .set_block((5, 5, 22), DummyWorldContext::PRESET_TYPES[0])
            .build((0, 0));
        let slab = c.terrain.slab(0.into()).unwrap();

        let x = SlabVerticalSpace::discover(slab);

        for b in x.iter_blocks() {
            eprintln!("{}: {}", b.0, b.1)
        }

        // bottom of slab up to block
        assert_eq!(
            x.find_block_exact(SlabPosition::new_unchecked(3, 3, LocalSliceIndex::bottom())),
            Some(3)
        );

        // same as bottom space
        assert_eq!(x.below_at((3, 3)), 3);

        // between the 2 blocks
        assert_eq!(
            x.find_block_exact(SlabPosition::new_unchecked(
                3,
                3,
                LocalSliceIndex::new_unchecked(4)
            )),
            Some(3)
        );

        // above the top block
        assert_eq!(
            x.find_block_exact(SlabPosition::new_unchecked(
                3,
                3,
                LocalSliceIndex::new_unchecked(8)
            )),
            Some(ABSOLUTE_MAX_FREE_VERTICAL_SPACE)
        );

        // really high block
        assert_eq!(x.above_at((3, 3)), 2);

        // ceiling block
        assert_eq!(x.above_at((4, 4)), 0);

        // open space
        assert_eq!(x.below_at((6, 7)), ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
        assert_eq!(x.above_at((6, 7)), ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
    }

    #[test]
    fn entire_empty_slab() {
        let c = ChunkBuilder::<DummyWorldContext>::new().build((0, 0));
        let slab = c.terrain.slab(0.into()).unwrap();

        let x = SlabVerticalSpace::discover(slab);
        assert!(x
            .iter_blocks()
            .all(|(_, h)| h == ABSOLUTE_MAX_FREE_VERTICAL_SPACE));

        assert_eq!(x.below_at((0, 0)), ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
        assert_eq!(x.below_at((5, 5)), ABSOLUTE_MAX_FREE_VERTICAL_SPACE);

        let shared_empty = SlabVerticalSpace::empty();
        assert_eq!(shared_empty, x);
    }

    #[test]
    fn entire_full_slab() {
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .fill_range(
                (0, 0, 0),
                (
                    CHUNK_SIZE.as_i32() - 1,
                    CHUNK_SIZE.as_i32() - 1,
                    SLAB_SIZE.as_i32() - 1,
                ),
                |_| DummyBlockType::Dirt,
            )
            .build((0, 0));
        let slab = c.terrain.slab(0.into()).unwrap();

        let x = SlabVerticalSpace::discover(slab);
        assert_eq!(x.iter_blocks().count(), 0);
        assert_eq!(x.above_at((5, 5)), 0);
        assert_eq!(x.below_at((5, 5)), 0);
    }

    #[test]
    fn above_top_slice() {
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .set_block((5, 5, SLAB_SIZE.as_i32() - 1), DummyBlockType::Dirt)
            .set_block((4, 5, SLAB_SIZE.as_i32() - 2), DummyBlockType::Dirt)
            .build((0, 0));
        let slab = c.terrain.slab(0.into()).unwrap();

        let x = SlabVerticalSpace::discover(slab);

        assert_eq!(x.above_at((3, 5)), ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
        assert_eq!(x.above_at((4, 5)), 1);
        assert_eq!(x.above_at((5, 5)), 0);
    }

    #[test]
    fn just_slice_0() {
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .fill_slice(0, DummyBlockType::Dirt)
            .set_block((5, 5, 0), DummyBlockType::Air) // hole
            .build((0, 0));
        let slab = c.terrain.slab(0.into()).unwrap();

        let x = SlabVerticalSpace::discover(slab);
        assert_eq!(x.below_at((5, 4)), 0); // solid
        assert_eq!(x.below_at((5, 5)), ABSOLUTE_MAX_FREE_VERTICAL_SPACE); // air
    }

    #[test]
    fn link_up_later_loaded_slabs_sideways_step() {
        let mut loader = loader_from_chunks_blocking_with_load_blacklist(
            vec![
                ChunkBuilder::new()
                    .fill_slice(3, DummyBlockType::Dirt)
                    .build((0, 0)),
                ChunkBuilder::new()
                    .fill_slice(4, DummyBlockType::Dirt)
                    .build((1, 0)),
            ],
            vec![SlabLocation::new(0, (1, 0))],
        );

        loader.request_slabs(once(SlabLocation::new(0, (1, 0))));
        loader.block_until_all_done(Duration::from_secs(2)).unwrap();

        let w = loader.world();

        let w = w.borrow();
        let edges = w.nav_graph().iter_inter_slab_edges().collect_vec();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0.chunk_idx, ChunkLocation(1, 0)); // from second slab to be loaded
    }

    #[test]
    fn link_up_later_loaded_slabs_sideways_flat() {
        let mut loader = loader_from_chunks_blocking_with_load_blacklist(
            vec![
                ChunkBuilder::new()
                    .fill_slice(3, DummyBlockType::Dirt)
                    .build((0, 0)),
                ChunkBuilder::new()
                    .fill_slice(3, DummyBlockType::Dirt)
                    .build((1, 0)),
            ],
            vec![SlabLocation::new(0, (1, 0))],
        );

        loader.request_slabs(once(SlabLocation::new(0, (1, 0))));
        loader.block_until_all_done(Duration::from_secs(2)).unwrap();

        let w = loader.world();

        let w = w.borrow();
        let edges = w.nav_graph().iter_inter_slab_edges().collect_vec();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn link_up_later_loaded_slabs() {
        let mut loader = loader_from_chunks_blocking_with_load_blacklist(
            vec![ChunkBuilder::new()
                .set_block((2, 0, -1), DummyBlockType::Dirt)
                .set_block((1, 0, -2), DummyBlockType::Dirt)
                .set_block((0, 0, -2), DummyBlockType::Dirt)
                .build((0, 0))],
            vec![SlabLocation::new(-1, (0, 0))],
        );

        loader.request_slabs(once(SlabLocation::new(-1, (0, 0))));
        loader.block_until_all_done(Duration::from_secs(2)).unwrap();

        let w = loader.world();

        let w = w.borrow();
        let chunk = w.find_chunk_with_pos(ChunkLocation(0, 0)).unwrap();

        let hi = WorldPosition::from((2, 0, 0));
        let lo = WorldPosition::from((1, 0, -1));
        let get_area = |b: WorldPosition| {
            chunk
                .find_area_for_block_with_height(b.into(), NavRequirement::ZERO)
                .unwrap_or_else(|| panic!("no area for block at {b}"))
        };

        let get_vs = |b: WorldPosition| {
            let pos = BlockPosition::from(b);
            chunk
                .slab_vertical_space(b.slice().slab_index())
                .unwrap_or_else(|| panic!("no slab vs for {b}"))
                .above_at(pos.xy())
        };

        // should protrude into slab above
        assert_eq!(
            get_area((0, 0, -1).into()).1.height,
            ABSOLUTE_MAX_FREE_VERTICAL_SPACE
        );
        assert_eq!(get_vs((0, 0, -1).into()), 1); // but vs is not updated from above

        // should both have areas
        let lo_area = get_area(lo);
        let hi_area = get_area(hi);

        assert_eq!(lo_area.1.height, ABSOLUTE_MAX_FREE_VERTICAL_SPACE);
        assert_eq!(hi_area.1.height, ABSOLUTE_MAX_FREE_VERTICAL_SPACE);

        let g = w.nav_graph();
        let edges = g.iter_inter_slab_edges().collect_vec();
        // let c = ChunkLocation(0,0);
        // assert_eq!(edges.len(), 1, "edges: {:?}", edges);
        // assert_eq!(edges, vec![(
        //     lo_area.0.to_chunk_area(SlabIndex(-1)).to_world_area(c),
        //     hi_area.0.to_chunk_area(SlabIndex(0)).to_world_area(c),
        //     SlabNavEdge {
        //         clearance: (),
        //         height_diff: 0,
        //     }
        //
        //
        //     )]);
    }

    #[test]
    fn cross_slab() {
        let w = world_from_chunks_blocking(vec![ChunkBuilder::new()
            // block at 30, 1 space at 31
            // block at 34, space should be 31 32 33
            .set_block((5, 5, SLAB_SIZE.as_i32() - 2), DummyBlockType::Dirt) // has 1 above in its slab
            .set_block((5, 5, SLAB_SIZE.as_i32() + 2), DummyBlockType::Dirt) // should have 3 below it across slab border
            // block at 63
            // block at 67, so space is 64 65 66
            .set_block((5, 5, (SLAB_SIZE.as_i32() * 2) - 1), DummyBlockType::Dirt) // top of next slab, floor of slab above it
            .set_block((5, 5, (SLAB_SIZE.as_i32() * 2) + 3), DummyBlockType::Dirt) // should have 3 below it across slab border too
            .set_block((5, 5, (SLAB_SIZE.as_i32() * 3) - 1), DummyBlockType::Dirt)
            .set_block((5, 5, (SLAB_SIZE.as_i32() * 3)), DummyBlockType::Dirt)
            .build((0, 0))]);

        let w = w.borrow();
        let chunk = w.find_chunk_with_pos(ChunkLocation(0, 0)).unwrap();

        let get_area = |z| {
            chunk
                .find_area_for_block_with_height(
                    BlockPosition::new_unchecked(5, 5, GlobalSliceIndex::new(z)),
                    NavRequirement::ZERO,
                )
                .unwrap_or_else(|| panic!("no area for block at z={z}"))
                .1
        };

        // top slice set from slab above
        let a = get_area(SLAB_SIZE.as_i32() - 1);
        assert_eq!(a.height, 3);

        // bottom slice set from slab below
        let a = get_area(SLAB_SIZE.as_i32() * 2);
        assert_eq!(a.height, 3);
    }

    #[test]
    fn find_block_downwards() {
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .set_block((2, 2, 0), DummyBlockType::Dirt)
            .set_block((2, 2, 4), DummyBlockType::Dirt)
            .set_block((2, 2, 5), DummyBlockType::Dirt)
            .build((0, 0));
        let slab = c.terrain.slab(0.into()).unwrap();

        let x = SlabVerticalSpace::discover(slab);

        let get_slice = |z| {
            println!("looking up z={z}");
            x.find_slice(SlabPosition::new_unchecked(
                2,
                2,
                LocalSliceIndex::new_unchecked(z),
            ))
            .map(|s| s.slice())
        };
        assert_eq!(get_slice(0), None); // solid

        for z in 1..=3 {
            assert_eq!(get_slice(z), Some(1));
        }

        assert_eq!(get_slice(4), None);
        assert_eq!(get_slice(5), None);

        // the space above goes up to the max
        for z in 6..6 + ABSOLUTE_MAX_FREE_VERTICAL_SPACE {
            assert_eq!(get_slice(z), Some(6));
        }

        // should not search further than the max
        for z in 6 + ABSOLUTE_MAX_FREE_VERTICAL_SPACE..SLAB_SIZE.as_u8() {
            assert_eq!(get_slice(z), None);
        }
    }
}
