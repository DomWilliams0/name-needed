use std::iter::once;
use std::ops::Deref;

use common::Itertools;
use unit::dim::CHUNK_SIZE;
use unit::world::{LocalSliceIndex, SlabIndex, SlabPosition, SliceBlock, SLAB_SIZE};

use crate::block::{Block, BlockType};
use crate::chunk::slice::{Slice, SliceMut};
use crate::{
    grid::{Grid, GridImpl},
    grid_declare,
};

grid_declare!(struct SlabGrid<SlabGridImpl, Block>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

#[derive(Clone)]
pub(crate) struct Slab {
    grid: SlabGrid,
    // TODO does a slab really need to know its index?
    index: SlabIndex,
}

impl Slab {
    pub fn empty<I: Into<SlabIndex>>(index: I) -> Self {
        Self {
            grid: SlabGrid::default(),
            index: index.into(),
        }
    }

    pub fn slice<S: Into<LocalSliceIndex>>(&self, index: S) -> Slice {
        let index = index.into();
        let (from, to) = SlabGrid::slice_range(index.slice());
        Slice::new(&(*self.grid)[from..to])
    }

    pub fn slice_mut<S: Into<LocalSliceIndex>>(&mut self, index: S) -> SliceMut {
        let index = index.into();
        let (from, to) = SlabGrid::slice_range(index.slice());
        SliceMut::new(&mut (*self.grid)[from..to])
    }

    /// (slice index *relative to this slab*, slice)
    pub fn slices_from_bottom(&self) -> impl DoubleEndedIterator<Item = (LocalSliceIndex, Slice)> {
        LocalSliceIndex::slices().map(move |idx| (idx, self.slice(idx)))
    }

    pub const fn block_count() -> usize {
        SlabGrid::FULL_SIZE
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
        let first = below.map(|slab| SliceSource::BelowSlab(slab.slice(LocalSliceIndex::top())));
        let middle = self
            .slices_from_bottom()
            .map(|(_, slice)| Some(SliceSource::ThisSlab(slice)));
        let last = above.map(|slab| SliceSource::AboveSlab(slab.slice(LocalSliceIndex::bottom())));

        once(first).chain(middle).chain(once(last)).tuple_windows()
    }

    pub(crate) fn set_block_type(&mut self, pos: SlabPosition, block_type: BlockType) {
        let slice_block = SliceBlock::from(pos);
        *self.slice_mut(pos.z())[slice_block].block_type_mut() = block_type;
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
