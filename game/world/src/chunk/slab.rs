use crate::block::Block;
use crate::chunk::slice::{Slice, SliceMut};
use crate::chunk::CHUNK_SIZE;
use crate::{
    grid::{Grid, GridImpl},
    grid_declare,
};
use unit::dim::SmallUnsignedConstant;
use unit::world::SliceIndex;

pub(crate) const SLAB_SIZE: SmallUnsignedConstant = SmallUnsignedConstant::new(32);

pub(crate) type SlabIndex = i32;

grid_declare!(struct SlabGrid<SlabGridImpl, Block>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

pub(crate) struct Slab {
    grid: SlabGrid,
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

    // pub(crate) fn collider_mut(&mut self) -> &mut SlabCollider {
    //     &mut self.collider
    // }

    /*
        pub fn highest_slice_index(&self) -> Option<SliceIndex> {
            (0..Self::slice_count()).rev()
               .filter(|&z| !self.slice(z).all_blocks_are(BlockType::Air))
               .next() // first
               .map(|i| i.into())
        }

        pub fn lowest_slice_index(&self) -> Option<SliceIndex> {
            (0..Self::slice_count())
                .filter(|&z| !self.slice(z).all_blocks_are(BlockType::Air))
                .next() // first
                .map(|i| i.into())
        }
    */
}
