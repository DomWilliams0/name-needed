use std::cell::Cell;
use std::iter::once;
use std::mem::MaybeUninit;
use std::ops::{Deref, Index};

use misc::*;
use unit::world::{
    BlockCoord, LocalSliceIndex, SlabIndex, SlabLocation, SlabPosition, SliceBlock, SliceIndex,
    WorldRange, SLAB_SIZE,
};
use unit::world::{LocalSliceIndexBelowTop, CHUNK_SIZE};

use crate::block::{Block, BlockOpacity};
use crate::chunk::slice::{unflatten_index, Slice, SliceMut, SliceOwned};
use crate::chunk::slice_navmesh::{
    make_mesh, SlabVerticalSpace, SliceAreaIndex, SliceConfig, ABSOLUTE_MAX_FREE_VERTICAL_SPACE,
};
use crate::loader::{FreeVerticalSpace, GenericTerrainUpdate, SlabTerrainUpdate};
use crate::navigation::discovery::AreaDiscovery;
use crate::navigation::{BlockGraph, ChunkArea};
use crate::occlusion::{BlockOcclusion, NeighbourOpacity, OcclusionFace};
use crate::{flatten_coords, BlockType, WorldChangeEvent, WorldContext, SLICE_SIZE};
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

    fn new_empty(ty: SlabType) -> Self {
        Self::from_grid(SlabGrid::default(), ty)
    }

    pub fn from_grid(grid: SlabGrid<C>, ty: SlabType) -> Self {
        let terrain = grid.into_boxed_impl();
        let arc = Arc::from(terrain);
        Self(arc, ty)
    }

    pub fn from_other_grid<G, T>(other: Grid<G>, ty: SlabType, conv: T) -> Self
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
        let (from, to) = self.slice_range(index.slice_unsigned());
        Slice::new(&self.array()[from..to])
    }

    pub fn slice_mut<S: Into<LocalSliceIndex>>(&mut self, index: S) -> SliceMut<C> {
        let index = index.into();
        let (from, to) = self.slice_range(index.slice_unsigned());
        SliceMut::new(&mut self.expect_mut().array_mut()[from..to])
    }

    /// (slice index *relative to this slab*, slice)
    pub fn slices_from_bottom(
        &self,
    ) -> impl DoubleEndedIterator<Item = (LocalSliceIndex, Slice<C>)> {
        LocalSliceIndex::slices().map(move |idx| (idx, self.slice(idx)))
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

#[derive(Debug, Copy, Clone)]
pub struct SliceNavArea {
    pub slice: u8,
    pub from: (BlockCoord, BlockCoord),
    /// Inclusive
    pub to: (BlockCoord, BlockCoord),
    pub area: SliceAreaIndex,
    pub height: FreeVerticalSpace,
}

struct NavmeshCfg<'a> {
    vec: &'a mut Vec<SliceNavArea>,
    cur_slice: u8,
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
            area: SliceAreaIndex(area),
            height,
        })
    }
}

/// Initialization functions
impl<C: WorldContext> Slab<C> {
    pub(crate) fn discover_navmesh(
        &self,
        vertical_space: &SlabVerticalSpace,
        above: Option<&SlabVerticalSpace>,
    ) -> Vec<SliceNavArea> {
        let maximum: u8 = 4; // TODO pass through
        let mut rects = vec![];
        let mut cfg = NavmeshCfg {
            vec: &mut rects,
            cur_slice: 1, // first slice is skipped
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

            if above.is_some() {
                // check if this goes to the top of the slab, so if we need above
                let remaining_until_top = LocalSliceIndex::top().slice() - slice_idx.slice();
                let remaining_until_top = remaining_until_top as u8;
                if remaining_until_top < maximum {
                    let above = unsafe { above.unwrap_unchecked() }; // checked already
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
            cfg.cur_slice = slice_idx.slice_unsigned() as u8;
            make_mesh(&mut cfg, &input, &mut output, &mut initialised);
        }

        rects
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
            cur_slice: 0,
        };
        let mut initialised = [false; SLICE_SIZE];

        #[allow(clippy::uninit_assumed_init, invalid_value)]
        // uninit values will not be ready anyway
        let mut output: [SliceAreaIndex; SLICE_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        make_mesh(&mut cfg, &input, &mut output, &mut initialised);
    }

    fn init_occlusion(&mut self, slice_above: Option<Slice<C>>, slice_below: Option<Slice<C>>) {
        // TODO sucks to do this because we cant mutate the block directly while iterating
        let mut occlusion_updates = vec![];
        self.ascending_slice_window(
            slice_above,
            slice_below,
            |slice_below, mut slice_this, slice_above| {
                for (i, b) in slice_this.iter().enumerate() {
                    let this_block = b.opacity();
                    if this_block.transparent() {
                        // TODO if leaving alone, ensure default is correct
                        continue;
                    }

                    let pos = unflatten_index(i);

                    let mut block_occlusion = *b.occlusion();

                    for face in OcclusionFace::FACES {
                        use OcclusionFace::*;

                        // extend in direction of face
                        let sideways_neighbour_pos = face.extend_sideways(pos);

                        // check if totally occluded
                        let neighbour_opacity = match face {
                            Top => slice_above.map(|s| (*s)[i].opacity()),
                            North => sideways_neighbour_pos.map(|pos| slice_this[pos].opacity()),
                            East => sideways_neighbour_pos.map(|pos| slice_this[pos].opacity()),
                            South => sideways_neighbour_pos.map(|pos| slice_this[pos].opacity()),
                            West => sideways_neighbour_pos.map(|pos| (&slice_this)[pos].opacity()),
                        };

                        let neighbour_opacity = if let Some(BlockOpacity::Solid) = neighbour_opacity
                        {
                            // totally occluded
                            NeighbourOpacity::all_solid()
                        } else if let Top = face {
                            // special case, top face only needs the slice above
                            if let Some(slice_above) = slice_above {
                                NeighbourOpacity::with_slice_above(pos, slice_above)
                            } else {
                                // no chunk above
                                NeighbourOpacity::unknown()
                            }
                        } else if let Some(relative_pos) = sideways_neighbour_pos {
                            NeighbourOpacity::with_neighbouring_slices(
                                relative_pos,
                                &slice_this,
                                slice_below,
                                slice_above,
                                face,
                            )
                        } else {
                            // missing chunk
                            NeighbourOpacity::unknown()
                        };

                        block_occlusion.set_face(face, neighbour_opacity);
                    }

                    occlusion_updates.push((i, block_occlusion));
                }

                for &(i, occ) in &occlusion_updates {
                    // safety: indices were just calculated above
                    unsafe {
                        *slice_this.get_unchecked_mut(i).occlusion_mut() = occ;
                    }
                }

                occlusion_updates.clear();
            },
        );
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

    /// Applies nav areas to blocks. Probably stored in the chunk in future instead
    pub(crate) fn apply_navigation_updates(&mut self, updates: &[SliceNavArea], replace_all: bool) {
        if updates.is_empty() {
            return;
        }
        debug!("applying nav updates to slab"; "n" => updates.len(), "replace_all" => replace_all);

        for (slice_idx, group) in &updates.iter().group_by(|u| u.slice) {
            let mut slice = self.slice_mut(LocalSliceIndex::new_unchecked(slice_idx as i32));

            if replace_all {
                slice.iter_mut().for_each(|b| b.clear_nav_area());
            }
            for u in group {
                for x in u.from.0..=u.to.0 {
                    for y in u.from.1..=u.to.1 {
                        let b = &mut slice[SliceBlock::new_srsly_unchecked(x, y)];
                        debug_assert!(
                            b.block_type().is_air(),
                            "{x},{y},{slice_idx} should be air but is {:?}",
                            b.block_type()
                        );
                        b.set_nav_area(u.area);
                    }
                }
            }
        }
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
    use crate::helpers::DummyWorldContext;
    use crate::DeepClone;

    #[test]
    fn deep_clone() {
        let a = Slab::<DummyWorldContext>::empty();
        let b = a.clone();
        let c = a.deep_clone();

        assert!(std::ptr::eq(a.raw(), b.raw()));
        assert!(!std::ptr::eq(a.raw(), c.raw()));
    }
}
