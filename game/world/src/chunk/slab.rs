use std::iter::once;
use std::ops::Deref;

use common::Itertools;
use unit::dim::{SmallUnsignedConstant, CHUNK_SIZE};
use unit::world::SliceIndex;

use crate::block::Block;
use crate::chunk::slice::{Slice, SliceMut};
use crate::{
    grid::{Grid, GridImpl},
    grid_declare,
};

pub(crate) const SLAB_SIZE: SmallUnsignedConstant = SmallUnsignedConstant::new(32);

pub(crate) type SlabIndex = i32;

grid_declare!(struct SlabGrid<SlabGridImpl, Block>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

#[derive(Clone)]
pub(crate) struct Slab {
    grid: SlabGrid,
    // TODO does a slab really need to know this?
    index: SlabIndex,
    // collider: SlabCollider,
}

impl Slab {
    pub fn empty(index: SlabIndex) -> Self {
        Self {
            grid: SlabGrid::default(),
            index,
            // collider: SlabCollider::default(),
        }
    }

    pub fn slice<S: Into<SliceIndex>>(&self, index: S) -> Slice {
        let SliceIndex(index) = index.into();
        let (from, to) = SlabGrid::slice_range(index);
        Slice::new(&(*self.grid)[from..to])
    }

    pub fn slice_mut<S: Into<SliceIndex>>(&mut self, index: S) -> SliceMut {
        let SliceIndex(index) = index.into();
        let (from, to) = SlabGrid::slice_range(index);
        SliceMut::new(&mut (*self.grid)[from..to])
    }

    /// (slice index *relative to this slab*, slice)
    pub fn slices_from_bottom(&self) -> impl DoubleEndedIterator<Item = (SliceIndex, Slice)> {
        (0..Self::slice_count()).map(move |idx| (SliceIndex(idx), self.slice(idx)))
    }

    pub const fn block_count() -> usize {
        SlabGrid::FULL_SIZE
    }

    pub const fn slice_count() -> i32 {
        SlabGrid::SLICE_COUNT
    }

    pub(crate) const fn index(&self) -> SlabIndex {
        self.index
    }

    pub(crate) fn grid(&self) -> &SlabGrid {
        &self.grid
    }

    pub(crate) fn grid_mut(&mut self) -> &mut SlabGrid {
        &mut self.grid
    }

    // (below sliceN, this slice0, this slice1), (this slice0, this slice1, this slice2) ...
    // (this sliceN-1, this sliceN, above0)
    pub fn ascending_slice_triplets<'a>(
        &'a self,
        below: Option<&'a Self>,
        above: Option<&'a Self>,
    ) -> impl Iterator<
        Item = (
            Option<SliceSource<'a>>,
            Option<SliceSource<'a>>,
            Option<SliceSource<'a>>,
        ),
    > {
        let first = below.map(|slab| SliceSource::BelowSlab(slab.slice(SLAB_SIZE.as_i32() - 1)));
        let middle = self
            .slices_from_bottom()
            .map(|(_, slice)| Some(SliceSource::ThisSlab(slice)));
        let last = above.map(|slab| SliceSource::AboveSlab(slab.slice(0)));

        once(first).chain(middle).chain(once(last)).tuple_windows()
    }
}

#[derive(Clone)]
pub enum SliceSource<'a> {
    BelowSlab(Slice<'a>),
    ThisSlab(Slice<'a>),
    AboveSlab(Slice<'a>),
}

impl<'a> Deref for SliceSource<'a> {
    type Target = Slice<'a>;

    fn deref(&self) -> &Self::Target {
        match self {
            SliceSource::BelowSlab(s) => s,
            SliceSource::ThisSlab(s) => s,
            SliceSource::AboveSlab(s) => s,
        }
    }
}

impl SliceSource<'_> {
    pub fn relative_slab_index(self, this_slab: SlabIndex) -> SlabIndex {
        match self {
            SliceSource::BelowSlab(_) => this_slab - 1,
            SliceSource::ThisSlab(_) => this_slab,
            SliceSource::AboveSlab(_) => this_slab + 1,
        }
    }
}
