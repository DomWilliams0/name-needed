use std::iter::once;
use std::ops::Deref;

use common::*;
use unit::world::CHUNK_SIZE;
use unit::world::{LocalSliceIndex, SlabIndex, SlabLocation, SlabPosition, WorldRange, SLAB_SIZE};

use crate::block::Block;
use crate::chunk::slice::{unflatten_index, Slice, SliceMut, SliceOwned};
use crate::loader::{GenericTerrainUpdate, SlabTerrainUpdate};
use crate::navigation::discovery::AreaDiscovery;
use crate::navigation::{BlockGraph, ChunkArea};
use crate::occlusion::{BlockOcclusion, NeighbourOpacity};
use crate::WorldChangeEvent;
use grid::{grid_declare, Grid, GridImpl};
use std::sync::Arc;

grid_declare!(pub struct SlabGrid<SlabGridImpl, Block>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

#[derive(Copy, Clone)]
pub enum SlabType {
    Normal,

    /// All air placeholder that should be overwritten with actual terrain
    Placeholder,
}

/// CoW slab terrain
#[derive(Clone)]
pub struct Slab(Arc<SlabGridImpl>, SlabType);

#[derive(Default)]
pub(crate) struct SlabInternalNavigability(Vec<(ChunkArea, BlockGraph)>);

pub trait DeepClone {
    fn deep_clone(&self) -> Self;
}

impl Slab {
    pub fn empty() -> Self {
        Self::new_empty(SlabType::Normal)
    }

    pub fn empty_placeholder() -> Self {
        Self::new_empty(SlabType::Placeholder)
    }

    fn new_empty(ty: SlabType) -> Self {
        Self::from_grid(SlabGrid::default(), ty)
    }

    pub fn from_grid(grid: SlabGrid, ty: SlabType) -> Self {
        let terrain = grid.into_boxed_impl();
        let arc = Arc::from(terrain);
        Self(arc, ty)
    }

    pub fn from_other_grid<I, G>(other: Grid<G>, ty: SlabType) -> Self
    where
        for<'a> &'a I: Into<<SlabGridImpl as GridImpl>::Item>,
        G: GridImpl<Item = I>,
    {
        let new_vals = other.array().iter().map(|item| item.into());
        let terrain = SlabGridImpl::from_iter(new_vals);
        let arc = Arc::from(terrain);
        Self(arc, ty)
    }

    pub fn cow_clone(&mut self) -> &mut Slab {
        let _ = Arc::make_mut(&mut self.0);
        self
    }

    pub fn expect_mut(&mut self) -> &mut SlabGridImpl {
        let grid = Arc::get_mut(&mut self.0).expect("expected to be the only slab reference");

        if let SlabType::Placeholder = std::mem::replace(&mut self.1, SlabType::Normal) {
            trace!("promoting placeholder slab to normal due to mutable reference");
        }

        grid
    }

    pub fn expect_mut_self(&mut self) -> &mut Slab {
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
    pub fn raw(&self) -> *const SlabGridImpl {
        Arc::into_raw(Arc::clone(&self.0))
    }

    pub fn slice<S: Into<LocalSliceIndex>>(&self, index: S) -> Slice {
        let index = index.into();
        let (from, to) = self.slice_range(index.slice_unsigned());
        Slice::new(&self.array()[from..to])
    }

    pub fn slice_mut<S: Into<LocalSliceIndex>>(&mut self, index: S) -> SliceMut {
        let index = index.into();
        let (from, to) = self.slice_range(index.slice_unsigned());
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

        Self(Arc::from(new_copy), self.1)
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
    pub(crate) fn process_terrain<'s>(
        &mut self,
        index: SlabIndex,
        above: Option<impl Into<Slice<'s>>>,
        below: Option<impl Into<Slice<'s>>>,
    ) -> SlabInternalNavigability {
        log_scope!(o!(index));

        // TODO detect when slab is all air and avoid expensive processing
        // but remember an all air slab above a solid slab DOES have an area on the first slice..

        // flood fill to discover navigability
        let navigation = self.discover_areas(index, below.map(Into::into));

        // occlusion
        self.init_occlusion(above.map(Into::into));

        navigation
    }

    fn discover_areas(
        &mut self,
        this_slab: SlabIndex,
        slice_below: Option<Slice>,
    ) -> SlabInternalNavigability {
        // TODO if exclusive we're in deep water with CoW
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

    pub(crate) fn apply_terrain_updates(
        &mut self,
        this_slab: SlabLocation,
        updates: impl Iterator<Item = SlabTerrainUpdate>,
        changes_out: &mut Vec<WorldChangeEvent>,
    ) {
        for update in updates {
            let GenericTerrainUpdate(range, block_type): SlabTerrainUpdate = update;
            trace!("setting blocks"; "range" => ?range, "type" => ?block_type);

            // TODO consider resizing/populating changes_out initially with empty events for performance
            match range {
                WorldRange::Single(pos) => {
                    let prev_block = self.slice_mut(pos.z()).set_block(pos, block_type);
                    let world_pos = pos.to_world_position(this_slab);
                    let event = WorldChangeEvent::new(world_pos, prev_block, block_type);
                    changes_out.push(event);
                }
                range @ WorldRange::Range(_, _) => {
                    let ((xa, xb), (ya, yb), (za, zb)) = range.ranges();
                    for z in za..=zb {
                        let mut slice = self.slice_mut(z);
                        // TODO reserve space in changes_out first
                        for x in xa..=xb {
                            for y in ya..=yb {
                                let prev_block = slice.set_block((x, y), block_type);
                                let world_pos =
                                    SlabPosition::new(x, y, z.into()).to_world_position(this_slab);
                                let event =
                                    WorldChangeEvent::new(world_pos, prev_block, block_type);
                                changes_out.push(event);
                            }
                        }
                    }
                }
            }
        }
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
