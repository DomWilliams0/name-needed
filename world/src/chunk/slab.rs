use std::iter::once;
use std::ops::Deref;

use misc::*;
use unit::world::{LocalSliceIndex, SlabIndex, SlabLocation, SlabPosition, WorldRange, SLAB_SIZE};
use unit::world::{LocalSliceIndexBelowTop, CHUNK_SIZE};

use crate::block::{Block, BlockOpacity};
use crate::chunk::slice::{unflatten_index, Slice, SliceMut, SliceOwned};
use crate::loader::{GenericTerrainUpdate, SlabTerrainUpdate};
use crate::navigation::discovery::AreaDiscovery;
use crate::navigation::{BlockGraph, ChunkArea};
use crate::occlusion::{BlockOcclusion, NeighbourOpacity, OcclusionFace};
use crate::{BlockType, WorldChangeEvent, WorldContext};
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
        let above = above.map(Into::into);
        let below = below.map(Into::into);

        log_scope!(o!(index));
        // TODO detect when slab is all air and avoid expensive processing
        // but remember an all air slab above a solid slab DOES have an area on the first slice..

        // flood fill to discover navigability
        let navigation = self.discover_areas(index, below);

        if navigation.0.is_empty() && matches!(self.1, SlabType::Placeholder) {
            // skip mutable references
        } else {
            // occlusion
            self.init_occlusion(above, below);
        }

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

        // skip expensive mutable reference cloning if no areas (empty slab)
        if area_count > 0 {
            // TODO discover internal area links

            // apply areas to blocks
            discovery.apply(self.expect_mut());
        }

        SlabInternalNavigability(slab_areas)
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
