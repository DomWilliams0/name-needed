use std::iter::once;
use std::ops::Deref;

use common::*;
use unit::dim::CHUNK_SIZE;
use unit::world::{LocalSliceIndex, SlabIndex, SlabLocation, SLAB_SIZE};

use crate::block::Block;
use crate::chunk::slice::{unflatten_index, Slice, SliceMut, SliceOwned};
use crate::navigation::discovery::AreaDiscovery;
use crate::navigation::{BlockGraph, ChunkArea};
use crate::neighbour::NeighbourOffset;
use crate::occlusion::{BlockOcclusion, NeighbourOpacity};
use grid::{grid_declare, Grid, GridImpl};
use std::sync::Arc;

grid_declare!(pub struct SlabGrid<SlabGridImpl, Block>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

/// CoW slab terrain
#[derive(Clone)]
#[repr(transparent)]
pub struct Slab(Arc<SlabGridImpl>);

pub(crate) struct SlabInternalNavigability(Vec<(ChunkArea, BlockGraph)>);

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
        // don't go via the stack to avoid overflow
        let mut new_copy = SlabGridImpl::default_boxed();
        new_copy.array.copy_from_slice(&self.array);

        Self(Arc::from(new_copy))
    }
}

impl Deref for Slab {
    type Target = SlabGridImpl;

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

/// Initialization functions
impl Slab {
    /// Discover navigability and occlusion
    pub(crate) fn process_terrain(
        &mut self,
        index: SlabIndex,
        above: Option<Slice>,
        below: Option<Slice>,
    ) -> SlabInternalNavigability {
        log_scope!(o!(index));

        // flood fill to discover navigability
        let navigation = self.discover_areas(index, below);

        // occlusion
        self.init_occlusion(above);

        navigation
    }

    fn discover_areas(
        &mut self,
        this_slab: SlabIndex,
        slice_below: Option<Slice>,
    ) -> SlabInternalNavigability {
        // TODO skip if not exclusive?
        // TODO exclusive helper on this struct too
        assert!(self.is_exclusive(), "not exclusive?");

        // collect slab into local grid
        let mut discovery = AreaDiscovery::from_slab(&self.0, this_slab, slice_below);

        // flood fill and assign areas
        let area_count = discovery.flood_fill_areas();
        debug!("discovered {count} areas", count = area_count);

        // collect areas and graphs
        let slab_areas = discovery.areas_with_graph().collect_vec();

        // TODO discover internal area links

        // apply areas to blocks
        discovery.apply(self.expect_mut());

        SlabInternalNavigability(slab_areas)
    }

    fn init_occlusion(&mut self, slice_above: Option<Slice>) {
        self.ascending_slice_pairs(slice_above, |mut slice_this, slice_next| {
            slice_this.iter_mut().enumerate().for_each(|(i, b)| {
                let this_block = b.opacity();
                let block_above = (*slice_next)[i].opacity();

                // this block should be solid and the one above it should not be
                let opacity = if this_block.solid() && block_above.transparent() {
                    let this_block = unflatten_index(i);

                    NeighbourOpacity::with_slice_above(this_block, slice_next)
                } else {
                    NeighbourOpacity::default()
                };

                *b.occlusion_mut() = BlockOcclusion::from_neighbour_opacities(opacity);
            });
        });
    }

    fn ascending_slice_pairs(
        &mut self,
        next_slab_up_bottom_slice: Option<Slice>,
        mut f: impl FnMut(SliceMut, Slice),
    ) {
        for (this_slice_idx, next_slice_idx) in LocalSliceIndex::slices().tuple_windows() {
            let this_slice_mut: SliceMut = self.slice_mut(this_slice_idx);

            // transmute lifetime to allow a mut and immut reference
            // safety: slices don't overlap and this_slice_idx != next_slice_idx
            let this_slice_mut: SliceMut = unsafe { std::mem::transmute(this_slice_mut) };
            let next_slice: Slice = self.slice(next_slice_idx);

            f(this_slice_mut, next_slice);
        }

        // top slice of this slab and bottom of next
        if let Some(next_slab_bottom_slice) = next_slab_up_bottom_slice {
            let this_slab_top_slice = self.slice_mut(LocalSliceIndex::top());

            // safety: mutable and immutable slices don't overlap
            let this_slab_top_slice: SliceMut = unsafe { std::mem::transmute(this_slab_top_slice) };

            f(this_slab_top_slice, next_slab_bottom_slice);
        }
    }

    pub fn slice_owned<S: Into<LocalSliceIndex>>(&self, index: S) -> SliceOwned {
        self.slice(index).to_owned()
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
