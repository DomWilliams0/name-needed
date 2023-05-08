use std::iter::once;
use std::mem::MaybeUninit;
use std::ops::Deref;

use misc::*;
use unit::world::CHUNK_SIZE;
use unit::world::{
    BlockCoord, LocalSliceIndex, SlabIndex, SlabLocation, SlabPosition, SliceBlock, SliceIndex,
    WorldRange, SLAB_SIZE,
};

use crate::block::{Block, BlockOpacity};
use crate::chunk::slice::{unflatten_index, Slice, SliceMut, SliceOwned};
use crate::chunk::slice_navmesh::{
    make_mesh, SlabVerticalSpace, SliceAreaIndex, SliceAreaIndexAllocator, SliceConfig,
};
use crate::loader::{FreeVerticalSpace, GenericTerrainUpdate, SlabTerrainUpdate};

use crate::navigation::{BlockGraph, ChunkArea};

use crate::occlusion::{NeighbourOpacity, OcclusionFace};
use crate::{flatten_coords, BlockType, WorldContext, SLICE_SIZE};
use grid::{Grid, GridImpl, GridImplExt};
use std::sync::Arc;

const GRID_DIM_X: usize = CHUNK_SIZE.as_usize();
const GRID_DIM_Y: usize = CHUNK_SIZE.as_usize();
const GRID_DIM_Z: usize = SLAB_SIZE.as_usize();

// manual expansion of grid_declare! to allow for generic parameter
pub type SlabGrid<C> = ::grid::Grid<SlabGridImpl<C>>;

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
#[repr(transparent)]
pub struct SlabGridImpl<C: WorldContext> {
    array: [Block<C>; GRID_DIM_X * GRID_DIM_Y * GRID_DIM_Z],
}

impl<C: WorldContext> ::grid::GridImpl for SlabGridImpl<C> {
    type Item = Block<C>;
    const DIMS: [usize; 3] = [GRID_DIM_X, GRID_DIM_Y, GRID_DIM_Z];
    const FULL_SIZE: usize = GRID_DIM_X * GRID_DIM_Y * GRID_DIM_Z;

    fn array(&self) -> &[Self::Item] {
        &self.array
    }

    fn array_mut(&mut self) -> &mut [Self::Item] {
        &mut self.array
    }
}

#[derive(Copy, Clone, Debug)]
pub enum SlabType {
    Normal,

    /// All air placeholder that should be overwritten with actual terrain
    Placeholder,
}

/// CoW slab terrain
#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct Slab<C: WorldContext>(Arc<SlabGridImpl<C>>, SlabType);

#[derive(Default)]
pub(crate) struct SlabInternalNavigability(Vec<(ChunkArea, BlockGraph)>);

pub(crate) struct SlabInternal {
    pub areas: Vec<SliceNavArea>,
}

pub trait DeepClone {
    fn deep_clone(&self) -> Self;
}

impl<C: WorldContext> Slab<C> {
    pub fn empty() -> Self {
        Self::new_empty(SlabType::Normal)
    }

    pub fn empty_placeholder() -> Self {
        Self::new_empty(SlabType::Placeholder)
    }

    pub fn all_stone() -> Self {
        Self(C::all_stone(), SlabType::Normal)
    }

    fn new_empty(ty: SlabType) -> Self {
        Self(C::all_air(), ty)
    }

    pub fn from_grid(grid: SlabGrid<C>, ty: SlabType) -> Self {
        let terrain = grid.into_boxed_impl();
        let arc = Arc::from(terrain);
        Self(arc, ty)
    }

    pub fn from_other_grid<G, T>(other: &Grid<G>, ty: SlabType, conv: T) -> Self
    where
        G: GridImpl,
        T: Fn(&G::Item) -> <SlabGridImpl<C> as GridImpl>::Item,
    {
        let new_vals = other.array().iter().map(conv);
        let terrain = SlabGridImpl::from_iter(new_vals);
        let arc = Arc::from(terrain);
        Self(arc, ty)
    }

    pub fn cow_clone(&mut self) -> &mut Slab<C> {
        let _ = Arc::make_mut(&mut self.0);
        self
    }

    pub fn expect_mut(&mut self) -> &mut SlabGridImpl<C> {
        let grid = Arc::get_mut(&mut self.0).expect("expected to be the only slab reference");

        if let SlabType::Placeholder = self.1 {
            self.1 = SlabType::Normal;
            trace!("promoting placeholder slab to normal due to mutable reference");
        }

        grid
    }

    pub fn expect_mut_self(&mut self) -> &mut Slab<C> {
        let _ = self.expect_mut();
        self
    }

    pub fn is_exclusive(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }

    pub fn is_placeholder(&self) -> bool {
        matches!(self.1, SlabType::Placeholder)
    }

    /// Leaks
    #[cfg(test)]
    pub fn raw(&self) -> *const SlabGridImpl<C> {
        Arc::into_raw(Arc::clone(&self.0))
    }

    pub fn slice<S: Into<LocalSliceIndex>>(&self, index: S) -> Slice<C> {
        let index = index.into();
        let (from, to) = SlabGridImpl::<C>::slice_range(index.slice_unsigned());
        Slice::new(&self.array()[from..to])
    }

    pub fn slice_mut<S: Into<LocalSliceIndex>>(&mut self, index: S) -> SliceMut<C> {
        let index = index.into();
        let (from, to) = SlabGridImpl::<C>::slice_range(index.slice_unsigned());
        SliceMut::new(&mut self.expect_mut().array_mut()[from..to])
    }

    pub fn slice_below(&self, slice: Slice<C>) -> Option<Slice<C>> {
        let src = slice.slice();
        if src.as_ptr() == self.array().as_ptr() {
            None
        } else {
            let slice_size = SlabGridImpl::<C>::SLICE_SIZE;
            unsafe {
                Some(Slice::new(std::slice::from_raw_parts(
                    src.as_ptr().offset(-(slice_size as isize)),
                    slice_size,
                )))
            }
        }
    }

    pub fn slice_above(&self, slice: Slice<C>) -> Option<Slice<C>> {
        let src = slice.slice();
        let slice_size = SlabGridImpl::<C>::SLICE_SIZE;
        let last_slice_ptr = unsafe {
            self.array()
                .as_ptr()
                .offset((slice_size * (SlabGridImpl::<C>::DIMS[2] - 1)) as isize)
        };
        if src.as_ptr() == last_slice_ptr {
            None
        } else {
            unsafe {
                Some(Slice::new(std::slice::from_raw_parts(
                    src.as_ptr().offset(slice_size as isize),
                    slice_size,
                )))
            }
        }
    }

    /// (slice index *relative to this slab*, slice)
    #[inline]
    pub fn slices_from_bottom(&self) -> impl Iterator<Item = (LocalSliceIndex, Slice<C>)> {
        let mut slice = Some(self.slice(LocalSliceIndex::bottom()));
        LocalSliceIndex::slices().zip(std::iter::from_fn(move || {
            let ret = slice;
            slice = slice.and_then(|slice| self.slice_above(slice));
            ret
        }))
    }

    /// (slice index *relative to this slab*, slice)
    pub fn slices_from_top(&self) -> impl DoubleEndedIterator<Item = (LocalSliceIndex, Slice<C>)> {
        LocalSliceIndex::slices()
            .rev()
            .map(move |idx| (idx, self.slice(idx)))
    }

    // (below sliceN, this slice0, this slice1), (this slice0, this slice1, this slice2) ...
    // (this sliceN-1, this sliceN, above0)
    pub fn ascending_slice_triplets<'a>(
        &'a self,
        below: Option<&'a Self>,
        above: Option<&'a Self>,
    ) -> impl Iterator<
        Item = (
            Option<SliceSource<'a, C>>,
            Option<SliceSource<'a, C>>,
            Option<SliceSource<'a, C>>,
        ),
    > {
        let first = below.map(|slab| SliceSource::BelowSlab(slab.slice(LocalSliceIndex::top())));
        let middle = self
            .slices_from_bottom()
            .map(|(_, slice)| Some(SliceSource::ThisSlab(slice)));
        let last = above.map(|slab| SliceSource::AboveSlab(slab.slice(LocalSliceIndex::bottom())));

        once(first).chain(middle).chain(once(last)).tuple_windows()
    }
}

impl<C: WorldContext> DeepClone for Slab<C> {
    fn deep_clone(&self) -> Self {
        // don't go via the stack to avoid overflow
        let mut new_copy = SlabGridImpl::default_boxed();
        new_copy.array.copy_from_slice(&self.array);

        Self(Arc::from(new_copy), self.1)
    }
}

impl<C: WorldContext> Deref for Slab<C> {
    type Target = SlabGridImpl<C>;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl IntoIterator for SlabInternalNavigability {
    type Item = (ChunkArea, BlockGraph);
    type IntoIter = std::vec::IntoIter<(ChunkArea, BlockGraph)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct SliceNavArea {
    pub slice: LocalSliceIndex,
    pub from: (BlockCoord, BlockCoord),
    /// Inclusive
    pub to: (BlockCoord, BlockCoord),
    // derive this from order in group of areas in the same slice
    // pub area: SliceAreaIndex,
    pub height: FreeVerticalSpace,
}

struct NavmeshCfg<'a> {
    vec: &'a mut Vec<SliceNavArea>,
    cur_slice: LocalSliceIndex,
}

impl SliceConfig for NavmeshCfg<'_> {
    const SZ: usize = CHUNK_SIZE.as_usize();
    const FILL_OUTPUT: bool = false; // unused
    type InputElem = u8;

    fn available_height(elem: Self::InputElem) -> u8 {
        elem
    }

    fn emit(&mut self, range: [usize; 2], height: u8, area: u8) {
        self.vec.push(SliceNavArea {
            slice: self.cur_slice,
            from: unflatten_index(range[0]).xy(),
            to: unflatten_index(range[1]).xy(),
            // area: SliceAreaIndex(area),
            height,
        })
    }
}

/// Initialization functions
impl<C: WorldContext> Slab<C> {
    pub(crate) fn discover_navmesh(
        &self,
        vertical_space: &SlabVerticalSpace,
        above: Option<&Arc<SlabVerticalSpace>>,
        out: &mut Vec<SliceNavArea>,
    ) {
        let maximum: u8 = 4; // TODO pass through
        let mut cfg = NavmeshCfg {
            vec: out,
            cur_slice: LocalSliceIndex::second_from_bottom(), // first slice is skipped
        };

        #[allow(clippy::uninit_assumed_init, invalid_value)]
        // uninit values will not be ready anyway
        let mut output: [SliceAreaIndex; SLICE_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        for (slice_idx, blocks) in &vertical_space
            .iter_blocks()
            .skip_while(|(pos, _)| pos.z() == LocalSliceIndex::bottom()) // skip bottom slice
            .group_by(|(pos, _)| pos.z())
        {
            log_scope!(o!("slice" => slice_idx.slice()));
            // setup input (uncapped)
            let mut input = [0u8; SLICE_SIZE];
            for (pos, h) in blocks {
                let idx = flatten_coords(SliceBlock::new_srsly_unchecked(pos.x(), pos.y()));
                unsafe {
                    *input.get_unchecked_mut(idx) = h;
                }
            }

            if let Some(above) = above {
                // check if this goes to the top of the slab, so if we need above
                let remaining_until_top =
                    (LocalSliceIndex::top().slice() - slice_idx.slice()) as u8;
                if remaining_until_top < maximum {
                    // in the top few slices, check each block if it extends into slab above
                    for (i, this) in input
                        .iter_mut()
                        .enumerate()
                        .filter(|(_, h)| **h > remaining_until_top)
                    {
                        // increase with air above
                        *this += above.below_at(unflatten_index(i).xy());
                        // TODO should post incremental updates to the slab above
                    }
                }
            }

            // clamp to max
            input.iter_mut().for_each(|h| *h = (*h).min(maximum));

            let mut initialised = [false; SLICE_SIZE];
            cfg.cur_slice = slice_idx;
            make_mesh(&mut cfg, &input, &mut output, &mut initialised);
        }
    }

    pub(crate) fn discover_bottom_slice_areas(
        &self,
        this: &SlabVerticalSpace,
        below: &SlabVerticalSpace,
        out: &mut Vec<SliceNavArea>,
    ) {
        let maximum = 4; // TODO pass through

        // setup input (uncapped)
        let mut input = [0u8; SLICE_SIZE];

        // only use blocks from this slab bottom slice if it is not solid
        let mut this_heights = [0; SLICE_SIZE];
        for (empty_pos, h) in this
            .iter_blocks()
            .take_while(|(pos, _)| pos.z() == LocalSliceIndex::bottom())
        {
            this_heights[flatten_coords(empty_pos.to_slice_block())] = h;
        }

        for (input, this_h, below) in
            izip!(input.iter_mut(), this_heights.iter(), below.above().iter(),)
        {
            if *this_h != 0 && *below == 0 {
                *input = (*this_h).min(maximum);
            }
        }

        let mut cfg = NavmeshCfg {
            vec: out,
            cur_slice: LocalSliceIndex::bottom(),
        };
        let mut initialised = [false; SLICE_SIZE];

        #[allow(clippy::uninit_assumed_init, invalid_value)]
        // uninit values will not be ready anyway
        let mut output: [SliceAreaIndex; SLICE_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        make_mesh(&mut cfg, &input, &mut output, &mut initialised);
    }

    pub fn init_occlusion(&mut self, slice_above: Option<Slice<C>>, slice_below: Option<Slice<C>>) {
        unreachable!()
    }

    /// f(maybe slice below, this slice, slice above)
    fn ascending_slice_window(
        &mut self,
        next_slab_up_bottom_slice: Option<Slice<C>>,
        prev_slab_top_slice: Option<Slice<C>>,
        mut f: impl FnMut(Option<Slice<C>>, SliceMut<C>, Option<Slice<C>>),
    ) {
        // top slice of prev slab and bottom of this one
        {
            let (this_slab_bottom_slice_idx, next_slice_idx) =
                LocalSliceIndex::slices().tuple_windows().next().unwrap();

            // transmute lifetime to allow a mut and immut references
            // safety: mutable and immutable slices don't overlap
            let this_slab_bottom_slice = unsafe {
                std::mem::transmute::<SliceMut<C>, SliceMut<C>>(
                    self.slice_mut(this_slab_bottom_slice_idx),
                )
            };

            let next_slice = self.slice(next_slice_idx);

            f(
                prev_slab_top_slice,
                this_slab_bottom_slice,
                Some(next_slice),
            );
        }

        for (prev_slice_idx, this_slice_idx, next_slice_idx) in
            LocalSliceIndex::slices().tuple_windows()
        {
            let this_slice_mut: SliceMut<C> = self.slice_mut(this_slice_idx);

            // transmute lifetime to allow a mut and immut references
            // safety: slices don't overlap and indices are distinct
            let this_slice_mut =
                unsafe { std::mem::transmute::<SliceMut<C>, SliceMut<C>>(this_slice_mut) };
            let prev_slice = self.slice(prev_slice_idx);
            let next_slice = self.slice(next_slice_idx);

            f(Some(prev_slice), this_slice_mut, Some(next_slice));
        }

        // top slice of this slab and optionally bottom of next
        {
            // safety: mutable and immutable slices don't overlap
            let this_slab_top_slice = unsafe {
                std::mem::transmute::<SliceMut<C>, SliceMut<C>>(
                    self.slice_mut(LocalSliceIndex::top()),
                )
            };
            let this_slab_below_top_slice = self.slice(
                LocalSliceIndex::slices_except_last()
                    .last()
                    .unwrap()
                    .current(),
            );
            f(
                Some(this_slab_below_top_slice),
                this_slab_top_slice,
                next_slab_up_bottom_slice,
            );
        }
    }

    pub fn slice_owned<S: Into<LocalSliceIndex>>(&self, index: S) -> SliceOwned<C> {
        self.slice(index).to_owned()
    }

    pub(crate) fn apply_terrain_updates(
        &mut self,
        this_slab: SlabLocation,
        updates: impl Iterator<Item = SlabTerrainUpdate<C>>,
    ) -> usize {
        let mut count = 0;
        for update in updates {
            let GenericTerrainUpdate(range, block_type): SlabTerrainUpdate<C> = update;
            trace!("setting blocks"; "range" => ?range, "type" => ?block_type);

            if let Some(pos) = range.as_single() {
                let _prev_block = self.slice_mut(pos.z()).set_block(pos, block_type);
                count += 1;
            } else {
                let ((xa, xb), (ya, yb), (za, zb)) = range.ranges();
                for z in za..=zb {
                    let z = LocalSliceIndex::new_unchecked(z);
                    let mut slice = self.slice_mut(z);
                    for x in xa..=xb {
                        for y in ya..=yb {
                            let _prev_block = slice.set_block((x, y), block_type);
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }
}

// ---------

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
#[allow(clippy::enum_variant_names)]
pub enum SliceSource<'a, C: WorldContext> {
    BelowSlab(Slice<'a, C>),
    ThisSlab(Slice<'a, C>),
    AboveSlab(Slice<'a, C>),
}

impl<'a, C: WorldContext> Deref for SliceSource<'a, C> {
    type Target = Slice<'a, C>;

    fn deref(&self) -> &Self::Target {
        match self {
            SliceSource::BelowSlab(s) => s,
            SliceSource::ThisSlab(s) => s,
            SliceSource::AboveSlab(s) => s,
        }
    }
}

impl<C: WorldContext> SliceSource<'_, C> {
    pub fn relative_slab_index(self, this_slab: SlabIndex) -> SlabIndex {
        match self {
            SliceSource::BelowSlab(_) => this_slab - 1,
            SliceSource::ThisSlab(_) => this_slab,
            SliceSource::AboveSlab(_) => this_slab + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::chunk::slab::Slab;
    use crate::chunk::slice::Slice;
    use crate::helpers::DummyWorldContext;
    use crate::DeepClone;
    use unit::world::{LocalSliceIndex, SliceIndex};

    #[test]
    fn deep_clone() {
        let a = Slab::<DummyWorldContext>::empty();
        let b = a.clone();
        let c = a.deep_clone();

        assert!(std::ptr::eq(a.raw(), b.raw()));
        assert!(!std::ptr::eq(a.raw(), c.raw()));
    }

    #[test]
    fn relative_slabs() {
        let a = Slab::<DummyWorldContext>::empty();
        for (idx, slice) in a.slices_from_bottom() {
            let above = a.slice_above(slice);
            let below = a.slice_below(slice);

            if idx == LocalSliceIndex::bottom() {
                assert!(below.is_none());
                assert!(above.is_some());
            } else if idx == LocalSliceIndex::top() {
                assert!(below.is_some());
                assert!(above.is_none())
            } else {
                assert!(below.is_some());
                assert!(above.is_some());
            }

            let cmp_slab = |slice: Slice<DummyWorldContext>, idx| {
                let cmp = a.slice(idx);
                assert_eq!(cmp.as_ptr_range(), slice.slice().as_ptr_range());
            };

            if let Some(above) = above {
                cmp_slab(above, LocalSliceIndex::new_unchecked(idx.slice() + 1));
            } else {
                assert_eq!(idx, LocalSliceIndex::top());
            }

            if let Some(below) = below {
                cmp_slab(below, LocalSliceIndex::new_unchecked(idx.slice() - 1));
            } else {
                assert_eq!(idx, LocalSliceIndex::bottom());
            }
        }
    }
}
