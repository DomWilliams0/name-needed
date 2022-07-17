use std::iter::once;
use std::ops::Deref;

use misc::*;
use unit::world::CHUNK_SIZE;
use unit::world::{LocalSliceIndex, SlabIndex, SlabLocation, SlabPosition, WorldRange, SLAB_SIZE};

use crate::block::Block;
use crate::chunk::slice::{unflatten_index, Slice, SliceMut, SliceOwned};
use crate::loader::{GenericTerrainUpdate, SlabTerrainUpdate};
use crate::navigation::discovery::AreaDiscovery;
use crate::navigation::{BlockGraph, ChunkArea};
use crate::occlusion::{BlockOcclusion, NeighbourOpacity};
use crate::{WorldChangeEvent, WorldContext};
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

#[derive(Copy, Clone)]
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

        if let SlabType::Placeholder = std::mem::replace(&mut self.1, SlabType::Normal) {
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

/// Initialization functions
impl<C: WorldContext> Slab<C> {
    /// Discover navigability and occlusion
    pub(crate) fn process_terrain<'s>(
        &mut self,
        index: SlabIndex,
        above: Option<impl Into<Slice<'s, C>>>,
        below: Option<impl Into<Slice<'s, C>>>,
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
        slice_below: Option<Slice<C>>,
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

    fn init_occlusion(&mut self, slice_above: Option<Slice<C>>) {
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
        next_slab_up_bottom_slice: Option<Slice<C>>,
        mut f: impl FnMut(SliceMut<C>, Slice<C>),
    ) {
        for (this_slice_idx, next_slice_idx) in LocalSliceIndex::slices().tuple_windows() {
            let this_slice_mut: SliceMut<C> = self.slice_mut(this_slice_idx);

            // transmute lifetime to allow a mut and immut reference
            // safety: slices don't overlap and this_slice_idx != next_slice_idx
            let this_slice_mut: SliceMut<C> = unsafe { std::mem::transmute(this_slice_mut) };
            let next_slice: Slice<C> = self.slice(next_slice_idx);

            f(this_slice_mut, next_slice);
        }

        // top slice of this slab and bottom of next
        if let Some(next_slab_bottom_slice) = next_slab_up_bottom_slice {
            let this_slab_top_slice = self.slice_mut(LocalSliceIndex::top());

            // safety: mutable and immutable slices don't overlap
            let this_slab_top_slice: SliceMut<C> =
                unsafe { std::mem::transmute(this_slab_top_slice) };

            f(this_slab_top_slice, next_slab_bottom_slice);
        }
    }

    pub fn slice_owned<S: Into<LocalSliceIndex>>(&self, index: S) -> SliceOwned<C> {
        self.slice(index).to_owned()
    }

    pub(crate) fn apply_terrain_updates(
        &mut self,
        this_slab: SlabLocation,
        updates: impl Iterator<Item = SlabTerrainUpdate<C>>,
        changes_out: &mut Vec<WorldChangeEvent<C>>,
    ) {
        for update in updates {
            let GenericTerrainUpdate(range, block_type): SlabTerrainUpdate<C> = update;
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
                        let z = LocalSliceIndex::new_unchecked(z);
                        let mut slice = self.slice_mut(z);
                        // TODO reserve space in changes_out first
                        for x in xa..=xb {
                            for y in ya..=yb {
                                let prev_block = slice.set_block((x, y), block_type);
                                let world_pos = SlabPosition::new_unchecked(x, y, z)
                                    .to_world_position(this_slab);
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
