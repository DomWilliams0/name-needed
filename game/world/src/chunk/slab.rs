use std::iter::once;
use std::ops::Deref;

use common::Itertools;
use unit::dim::CHUNK_SIZE;
use unit::world::{LocalSliceIndex, SlabIndex, SLAB_SIZE};

use crate::block::Block;
use crate::chunk::slice::{Slice, SliceMut};
use grid::{grid_declare, Grid, GridImpl};
use std::sync::Arc;

grid_declare!(pub struct SlabGrid<SlabGridImpl, Block>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

/// CoW
#[derive(Clone)]
#[repr(transparent)]
pub(crate) struct Slab(Arc<SlabGridImpl>);

pub trait DeepClone {
    fn deep_clone(&self) -> Self;
}

impl Slab {
    pub fn empty() -> Self {
        let terrain = SlabGrid::default().into_boxed_impl();
        let arc = Arc::from(terrain);
        Self(arc)
    }

    pub fn cow_clone(&mut self) -> &mut Slab {
        let _ = Arc::make_mut(&mut self.0);
        self
    }

    pub fn expect_mut(&mut self) -> &mut SlabGridImpl {
        Arc::get_mut(&mut self.0).expect("expected to be the only slab reference")
    }

    pub fn expect_mut_self(&mut self) -> &mut Slab {
        let _ = self.expect_mut();
        self
    }

    pub fn is_exclusive(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    /// Leaks
    #[cfg(test)]
    pub fn raw(&self) -> *const SlabGridImpl {
        Arc::into_raw(Arc::clone(&self.0))
    }

    pub fn slice<S: Into<LocalSliceIndex>>(&self, index: S) -> Slice {
        let index = index.into();
        let (from, to) = self.slice_range(index.slice());
        Slice::new(&self.array()[from..to])
    }

    pub fn slice_mut<S: Into<LocalSliceIndex>>(&mut self, index: S) -> SliceMut {
        let index = index.into();
        let (from, to) = self.slice_range(index.slice());
        SliceMut::new(&mut self.expect_mut().array_mut()[from..to])
    }

    /// (slice index *relative to this slab*, slice)
    pub fn slices_from_bottom(&self) -> impl DoubleEndedIterator<Item = (LocalSliceIndex, Slice)> {
        LocalSliceIndex::slices().map(move |idx| (idx, self.slice(idx)))
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
}

impl DeepClone for Slab {
    fn deep_clone(&self) -> Self {
        let grid: SlabGridImpl = (*self.0).clone();
        Self(Arc::from(grid))
    }
}

impl Deref for Slab {
    type Target = SlabGridImpl;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

// ---------

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

#[cfg(test)]
mod tests {
    use crate::chunk::slab::Slab;
    use crate::DeepClone;

    #[test]
    fn deep_clone() {
        let a = Slab::empty();
        let b = a.clone();
        let c = a.deep_clone();

        assert!(std::ptr::eq(a.raw(), b.raw()));
        assert!(!std::ptr::eq(a.raw(), c.raw()));
    }
}
