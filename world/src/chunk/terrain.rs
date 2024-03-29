use std::f32::EPSILON;
use std::hint::unreachable_unchecked;
use std::iter::{once, repeat};

use crate::block::BlockDurability;
use misc::*;
pub(crate) use pair_walking::WhichChunk;
use unit::world::{
    BlockCoord, BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex,
    SLAB_SIZE,
};
use unit::world::{SliceBlock, CHUNK_SIZE};

use crate::block::Block;
use crate::chunk::double_sided_vec::DoubleSidedVec;
use crate::chunk::slab::DeepClone;
use crate::chunk::slab::Slab;
use crate::chunk::slice::{Slice, SliceMut};

use crate::navigation::ChunkArea;
use crate::neighbour::NeighbourOffset;
use crate::occlusion::NeighbourOpacity;
use crate::{BlockType, EdgeCost, SliceRange, WorldContext};

/// Terrain only. Clone with `deep_clone`
pub struct RawChunkTerrain<C: WorldContext> {
    slabs: DoubleSidedVec<Slab<C>>,
}

pub trait BaseTerrain<C: WorldContext> {
    fn raw_terrain(&self) -> &RawChunkTerrain<C>;
    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain<C>;

    fn slice<S: Into<GlobalSliceIndex>>(&self, index: S) -> Option<Slice<C>> {
        let chunk_slice_idx = index.into();
        let slab_idx = chunk_slice_idx.slab_index();
        self.raw_terrain()
            .slabs
            .get(slab_idx)
            .map(|ptr| ptr.slice(chunk_slice_idx.to_local()))
    }

    fn slice_unchecked<S: Into<GlobalSliceIndex>>(&self, index: S) -> Slice<C> {
        // TODO actually add get_{mut_}unchecked to slabs for performance
        self.slice(index).unwrap()
    }

    /// Calls `Slab::expect_mut`, panics if not the exclusive reference
    fn slice_mut<S: Into<GlobalSliceIndex>>(&mut self, index: S) -> Option<SliceMut<C>> {
        let chunk_slice_idx = index.into();
        let slab_idx = chunk_slice_idx.slab_index();
        self.raw_terrain_mut()
            .slabs
            .get_mut(slab_idx)
            .map(|ptr| ptr.expect_mut_self().slice_mut(chunk_slice_idx.to_local()))
    }

    /// Calls `Slab::cow_clone`, triggering a slab copy if necessary
    fn slice_mut_with_cow<S: Into<GlobalSliceIndex>>(&mut self, index: S) -> Option<SliceMut<C>> {
        let chunk_slice_idx = index.into();
        let slab_idx = chunk_slice_idx.slab_index();
        self.raw_terrain_mut()
            .slabs
            .get_mut(slab_idx)
            .map(|ptr| ptr.cow_clone().slice_mut(chunk_slice_idx.to_local()))
    }

    /// Will clone CoW slab if necessary
    fn slice_mut_unchecked_with_cow<S: Into<GlobalSliceIndex>>(&mut self, index: S) -> SliceMut<C> {
        self.slice_mut_with_cow(index).unwrap()
    }

    fn get_block(&self, pos: BlockPosition) -> Option<Block<C>> {
        self.slice(pos.z()).map(|slice| slice[pos])
    }

    /// Panics if invalid position
    #[cfg(test)]
    fn get_block_tup(&self, pos: (i32, i32, i32)) -> Option<Block<C>> {
        let pos = BlockPosition::try_from(pos).expect("bad position");
        self.slice(pos.z()).map(|slice| slice[pos])
    }

    /// Returns the range of slices in this terrain rounded to the nearest slab
    fn slice_bounds_as_slabs(&self) -> SliceRange {
        let mut slabs = self.raw_terrain().slabs.indices_increasing();
        let bottom = slabs.next().unwrap_or(0);
        let top = slabs.last().unwrap_or(0) + 1;

        SliceRange::from_bounds_unchecked(bottom * SLAB_SIZE.as_i32(), top * SLAB_SIZE.as_i32())
    }

    /// Only for tests
    #[cfg(test)]
    fn blocks<'a>(
        &self,
        out: &'a mut Vec<(BlockPosition, Block<C>)>,
    ) -> &'a mut Vec<(BlockPosition, Block<C>)> {
        let (_bottom_slab, bottom_slab_index) =
            self.raw_terrain().slabs_from_bottom().next().unwrap();

        let SlabIndex(low_z) = bottom_slab_index * SLAB_SIZE;
        let high_z = low_z + (self.raw_terrain().slab_count() * SLAB_SIZE.as_usize()) as i32;

        let total_size =
            (high_z - low_z) as usize * (CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize());
        out.reserve(total_size);
        out.clear();

        let iter_from = if low_z != 0 { low_z + 1 } else { low_z };

        for z in iter_from..high_z {
            for y in 0..CHUNK_SIZE.as_block_coord() {
                for x in 0..CHUNK_SIZE.as_block_coord() {
                    let z = GlobalSliceIndex::new(z);
                    let pos = BlockPosition::new_unchecked(x, y, z.into());
                    let block = self.get_block(pos);
                    out.push((pos, block.unwrap()));
                }
            }
        }

        out
    }
}

pub struct OcclusionChunkUpdate(
    pub ChunkLocation,
    pub Vec<(BlockPosition, NeighbourOpacity)>,
);

#[derive(Copy, Clone)]
pub enum SlabCreationPolicy {
    /// Don't add missing slabs
    PleaseDont,

    /// Create the missing slab and all intermediate slabs
    CreateAll,
}

pub enum BlockDamageResult {
    Broken,
    Unbroken,
}

impl<C: WorldContext> BaseTerrain<C> for RawChunkTerrain<C> {
    fn raw_terrain(&self) -> &RawChunkTerrain<C> {
        self
    }

    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain<C> {
        self
    }
}

impl<C: WorldContext> RawChunkTerrain<C> {
    pub(crate) fn slabs_from_top(&self) -> impl Iterator<Item = (&Slab<C>, SlabIndex)> {
        self.slabs
            .iter_decreasing()
            .zip(self.slabs.indices_decreasing())
            .map(|(ptr, idx)| (ptr, SlabIndex(idx)))
    }

    pub(crate) fn slabs_from_bottom(&self) -> impl Iterator<Item = (&Slab<C>, SlabIndex)> {
        self.slabs
            .iter_increasing()
            .zip(self.slabs.indices_increasing())
            .map(|(ptr, idx)| (ptr, SlabIndex(idx)))
    }

    /// Adds slab, returning old if it exists
    pub fn replace_slab(&mut self, index: SlabIndex, new_slab: Slab<C>) -> Option<Slab<C>> {
        if let Some(old) = self.slabs.get_mut(index) {
            Some(std::mem::replace(old, new_slab))
        } else {
            self.slabs.add(new_slab, index);
            None
        }
    }

    #[cfg(test)]
    pub fn add_empty_placeholder_slab(&mut self, slab: impl Into<SlabIndex>) {
        self.slabs.add(Slab::empty_placeholder(), slab.into());
    }

    pub(crate) fn slab(&self, index: SlabIndex) -> Option<&Slab<C>> {
        self.slabs.get(index)
    }

    /// Cow-copies the slab if not already the exclusive holder
    pub(crate) fn slab_mut(&mut self, index: SlabIndex) -> Option<&mut Slab<C>> {
        self.slabs.get_mut(index).map(|s| s.cow_clone())
    }

    pub(crate) fn copy_slab(&self, index: SlabIndex) -> Option<Slab<C>> {
        self.slabs.get(index).map(|s| s.deep_clone())
    }

    /// Fills in gaps in slabs up to the inclusive target with empty placeholder slabs. Nop if zero
    pub(crate) fn create_slabs_until(&mut self, target: SlabIndex) {
        if target != SlabIndex(0) {
            self.slabs.fill_until(target, |_| Slab::empty_placeholder())
        }
    }

    pub fn slab_count(&self) -> usize {
        self.slabs.len()
    }

    /// Inclusive
    pub fn slab_range(&self) -> (SlabIndex, SlabIndex) {
        let (a, b) = self.slabs.index_range();
        (SlabIndex(a), SlabIndex(b))
    }

    /// Returns the range of slices in this terrain rounded to the nearest slab
    pub fn slice_bounds_as_slabs(&self) -> SliceRange {
        let mut slabs = self.slabs.indices_increasing();
        let bottom = slabs.next().unwrap_or(0);
        let top = slabs.last().unwrap_or(0) + 1;

        SliceRange::from_bounds_unchecked(bottom * SLAB_SIZE.as_i32(), top * SLAB_SIZE.as_i32())
    }

    pub fn slices_from_bottom(&self) -> impl Iterator<Item = (LocalSliceIndex, Slice<C>)> {
        self.slabs_from_bottom()
            .flat_map(|(slab, _)| slab.slices_from_bottom())
    }

    /// (global slice index, slice)
    pub fn slices_from_top_offset(&self) -> impl Iterator<Item = (GlobalSliceIndex, Slice<C>)> {
        self.slabs_from_top().flat_map(|(slab, idx)| {
            slab.slices_from_bottom()
                .rev()
                .map(move |(z, slice)| (z.to_global(idx), slice))
        })
    }

    pub fn slab_boundary_slices(
        &self,
    ) -> impl Iterator<Item = (GlobalSliceIndex, Slice<C>, Slice<C>)> {
        self.slabs
            .indices_increasing()
            .zip(self.slabs.iter_increasing())
            .tuple_windows()
            .map(|((lower_idx, lower), (_, upper))| {
                let lower_idx = LocalSliceIndex::top().to_global(SlabIndex(lower_idx));
                let lower_hi = lower.slice(LocalSliceIndex::top());
                let upper_lo = upper.slice(LocalSliceIndex::bottom());

                (lower_idx, lower_hi, upper_lo)
            })
    }

    /// If slab doesn't exist, does nothing and returns false
    pub fn try_set_block(&mut self, pos: BlockPosition, block: C::BlockType) -> bool {
        self.set_block(pos, block, SlabCreationPolicy::PleaseDont)
    }

    /// Returns if block was set successfully, depends on slab creation policy
    pub fn set_block(
        &mut self,
        pos: BlockPosition,
        block: C::BlockType,
        policy: SlabCreationPolicy,
    ) -> bool {
        let block = Block::with_block_type(block);
        self.slice_mut_with_policy(pos.z(), policy, |mut slice| slice[pos] = block)
    }

    pub fn slice_mut_with_policy<S: Into<GlobalSliceIndex>, F: FnOnce(SliceMut<C>)>(
        &mut self,
        slice: S,
        policy: SlabCreationPolicy,
        f: F,
    ) -> bool {
        let mut try_again = true;
        let slice = slice.into();

        loop {
            if let Some(slice) = self.slice_mut(slice) {
                // nice, slice exists: we're done
                f(slice);
                return true;
            }

            // slice doesn't exist

            match policy {
                SlabCreationPolicy::CreateAll if try_again => {
                    // create slabs
                    self.create_slabs_until(slice.slab_index());

                    // try again once more
                    try_again = false;
                    continue;
                }
                _ => return false,
            };
        }
    }

    pub fn with_block_mut_unchecked<F: FnMut(&mut Block<C>)>(
        &mut self,
        pos: BlockPosition,
        mut f: F,
    ) {
        let mut slice = self.slice_mut(pos.z()).unwrap();
        let block = &mut slice[pos];
        f(block);
    }

    pub(crate) fn apply_block_damage(
        &mut self,
        pos: BlockPosition,
        damage: BlockDurability,
    ) -> Option<BlockDamageResult> {
        if let Some(mut slice) = self.slice_mut(pos.z()) {
            let block = &mut slice[pos];
            let durability = block.durability_mut();
            *durability -= damage;

            Some(if durability.proportion() < EPSILON {
                BlockDamageResult::Broken
            } else {
                BlockDamageResult::Unbroken
            })
        } else {
            None
        }
    }

    fn limited_slab_indices(&self, limit: (SlabIndex, SlabIndex)) -> (SlabIndex, SlabIndex) {
        let (actual_min, actual_max) = self.slabs.index_range();
        let (req_min, req_max) = limit;

        let min = req_min.as_i32().max(actual_min);
        let max = req_max.as_i32().min(actual_max);
        (SlabIndex(min), SlabIndex(max))
    }

    /// offset: self->other
    pub(crate) fn cross_chunk_pairs_foreach<
        F: FnMut(WhichChunk, BlockPosition, NeighbourOpacity),
    >(
        &'_ self,
        other: &'_ Self,
        offset: NeighbourOffset,
        slab_range: (SlabIndex, SlabIndex),
        mut f: F,
    ) {
        let offset_opposite = offset.opposite();

        let yield_ = if offset.is_aligned() {
            pair_walking::yield_side
        } else {
            pair_walking::yield_corner
        };

        // find slab range
        let (my_min, my_max) = self.limited_slab_indices(slab_range);
        let (ur_min, ur_max) = other.limited_slab_indices(slab_range);

        // one chunk starts lower than the other
        if my_min != ur_min {
            let (lower, which_lower, higher, higher_min, dir) = if my_min < ur_min {
                (self, WhichChunk::ThisChunk, other, ur_min, offset)
            } else {
                (other, WhichChunk::OtherChunk, self, my_min, offset_opposite)
            };

            // skip lower slabs up until its the last one before the next
            let lower_slab_index = higher_min - 1;

            // compare top slice of this vs bottom slice of other
            let lower_slice_above = lower
                .slab(lower_slab_index + 1)
                .map(|slab| slab.slice(LocalSliceIndex::bottom()));

            let (_, bottom_slice) = higher.slices_from_bottom().next().unwrap();
            yield_(
                which_lower,
                lower_slice_above,
                lower_slab_index,
                LocalSliceIndex::top(),
                bottom_slice,
                dir,
                &mut f,
            )
        }

        // continue from the common min = max of the mins
        let first_misc_slab = my_min.max(ur_min);

        // yield slices up until first max
        let first_max = my_max.min(ur_max);

        for (slab_index, next_slab_index) in (first_misc_slab.as_i32()..=first_max.as_i32())
            .map(Some)
            .chain(once(None))
            .tuple_windows()
        {
            let slab_index = SlabIndex(slab_index.unwrap()); // always Some
            let this_slab = self.slab(slab_index).unwrap();
            let other_slab = other.slab(slab_index).unwrap();

            for z in LocalSliceIndex::slices_except_last() {
                let z_above = z.above();
                let z_current = z.current();

                let this_slice_above = this_slab.slice(z_above);
                let upper_slice = other_slab.slice(z_above);
                yield_(
                    WhichChunk::ThisChunk,
                    Some(this_slice_above),
                    slab_index,
                    z_current,
                    upper_slice,
                    offset,
                    &mut f,
                );

                let upper_slice = this_slab.slice(z_above);
                let other_slice_above = other_slab.slice(z_above);
                yield_(
                    WhichChunk::OtherChunk,
                    Some(other_slice_above),
                    slab_index,
                    z_current,
                    upper_slice,
                    offset_opposite,
                    &mut f,
                );
            }

            // special case of top slice of one and bottom slice of next
            if let Some(next_slab_index) = next_slab_index {
                let next_slab_index = SlabIndex(next_slab_index);
                let this_slice_above = self
                    .slab(next_slab_index)
                    .map(|slab| slab.slice(LocalSliceIndex::bottom()));
                let next_slice = other
                    .slab(next_slab_index)
                    .unwrap()
                    .slice(LocalSliceIndex::bottom());
                yield_(
                    WhichChunk::ThisChunk,
                    this_slice_above,
                    slab_index,
                    LocalSliceIndex::top(),
                    next_slice,
                    offset,
                    &mut f,
                );

                // let top_slice = other_slab.slice(SLAB_SIZE.as_i32() - 1);
                let other_slice_above = other
                    .slab(next_slab_index)
                    .map(|slab| slab.slice(LocalSliceIndex::bottom()));
                let next_slice = self
                    .slab(next_slab_index)
                    .unwrap()
                    .slice(LocalSliceIndex::bottom());
                yield_(
                    WhichChunk::OtherChunk,
                    other_slice_above,
                    slab_index,
                    LocalSliceIndex::top(),
                    next_slice,
                    offset_opposite,
                    &mut f,
                );
            }
        }

        // one chunk ends higher than the other
        if my_max != ur_max {
            let (higher, lower, which_lower, lower_max, dir) = if my_max > ur_max {
                (self, other, WhichChunk::OtherChunk, ur_max, offset)
            } else {
                (other, self, WhichChunk::ThisChunk, my_max, offset_opposite)
            };

            // top slice of lower and the bottom slice of next higher
            // let top_slice = lower.slab(lower_max).unwrap().slice(SLAB_SIZE.as_i32() - 1);
            let lower_slice_above = lower
                .slab(lower_max + 1)
                .map(|slab| slab.slice(LocalSliceIndex::bottom()));
            let bottom_slice = higher
                .slab(lower_max + 1)
                .unwrap()
                .slice(LocalSliceIndex::bottom());
            yield_(
                which_lower,
                lower_slice_above,
                lower_max,
                LocalSliceIndex::top(),
                bottom_slice,
                dir,
                &mut f,
            );

            // no need to bother with rest of higher slabs
        }
    }

    pub(crate) fn cross_chunk_pairs_nav_foreach<
        F: FnMut(ChunkArea, ChunkArea, EdgeCost, BlockCoord, GlobalSliceIndex),
    >(
        &'_ self,
        other: &'_ Self,
        offset: NeighbourOffset,
        slab_range: (SlabIndex, SlabIndex),
        mut f: F,
    ) {
        let (SlabIndex(start), SlabIndex(end)) = self.limited_slab_indices(slab_range);
        for slab_idx in start..=end {
            let my_slab = self.slabs.get(slab_idx).unwrap();

            // get loaded adjacent neighbour slab
            let ur_slab_adjacent = match other.slabs.get(slab_idx) {
                Some(s) => s,
                None => {
                    // skip this whole slab, no links to be made
                    continue;
                }
            };

            let ur_slab_below = other.slabs.get(slab_idx - 1);
            let ur_slab_above = other.slabs.get(slab_idx + 1);

            let mut coord_range = [(0, 0); CHUNK_SIZE.as_usize()];
            pair_walking::calculate_boundary_slice_block_offsets(offset, &mut coord_range);
            let x_coord_changes = offset.is_vertical();

            // iterate the boundary slices of this slab
            for ((slice_idx, slice), (ur_slice_below, ur_slice, ur_slice_above)) in my_slab
                .slices_from_bottom()
                .zip(ur_slab_adjacent.ascending_slice_triplets(ur_slab_below, ur_slab_above))
            {
                // only iterate blocks that are walkable on this side
                let slice_offsets = [
                    (ur_slice_below, EdgeCost::JumpDown),
                    (ur_slice, EdgeCost::Walk),
                    (ur_slice_above, EdgeCost::JumpUp),
                ];
                for (wx, wy, this_area) in coord_range
                    .iter()
                    .copied()
                    .filter_map(|(x, y)| slice[(x, y)].area_index().ok().map(|a| (x, y, a)))
                {
                    let ur_sliceblock = pair_walking::extend_boundary_slice_block(offset, (wx, wy));

                    // check below, adjacent and above in the other slab as applicable
                    for (ur_slice, &cost) in slice_offsets
                        .iter()
                        .filter_map(|(s, e)| s.clone().map(|s| (s, e)))
                    {
                        if let Some(other_area) = ur_slice[ur_sliceblock].area_index().ok() {
                            let slab_idx = SlabIndex(slab_idx);

                            // link found
                            let src = ChunkArea {
                                slab: slab_idx,
                                area: this_area,
                            };
                            let dst = ChunkArea {
                                slab: ur_slice.relative_slab_index(slab_idx),
                                area: other_area,
                            };

                            let coord = if x_coord_changes { wx } else { wy };
                            f(src, dst, cost, coord, slice_idx.to_global(slab_idx));

                            // done with this slice
                            // TODO could skip next slice because it cant be walkable if this one was?
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn deep_clone(&self) -> Self {
        Self {
            slabs: self.slabs.deep_clone(),
        }
    }

    /// Returns iter of slabs affected, probably duplicated but in order
    pub fn apply_occlusion_updates<'a>(
        &'a mut self,
        updates: &'a [(BlockPosition, NeighbourOpacity)],
    ) -> impl Iterator<Item = SlabIndex> + 'a {
        updates
            .iter()
            .filter_map(move |&(block_pos, new_opacities)| {
                let raw_terrain = self.raw_terrain_mut();

                // obtain immutable slice for checking, to avoid unnecessary CoW slab copying
                let slice = raw_terrain.slice_unchecked(block_pos.z());

                if *slice[block_pos].occlusion() != new_opacities {
                    // opacities have changed, promote slice to mutable, possibly triggering a slab copy
                    // TODO this is sometimes a false positive, triggering unnecessary copies
                    let block_mut =
                        &mut raw_terrain.slice_mut_unchecked_with_cow(block_pos.z())[block_pos];

                    // for trace logging only
                    let _old_occlusion = *block_mut.occlusion();

                    block_mut
                        .occlusion_mut()
                        .update_from_neighbour_opacities(new_opacities);

                    trace!(
                        "new AO for block";
                        "block" => ?block_pos,
                        "old" => ?_old_occlusion,
                        "new" => ?new_opacities,
                        "updated" => ?block_mut.occlusion()
                    );

                    let slab = block_pos.z().slab_index();
                    Some(slab)
                } else {
                    None
                }
            })
    }

    // TODO use an enum for the slice range rather than Options
    pub fn find_accessible_block(
        &self,
        pos: SliceBlock,
        start_from: Option<GlobalSliceIndex>,
        end_at: Option<GlobalSliceIndex>,
    ) -> Option<BlockPosition> {
        self.find_from_top(pos, start_from, end_at, |above, below| {
            above.walkable() && below.block_type().can_be_walked_on()
        })
    }

    pub fn find_ground_level(
        &self,
        pos: SliceBlock,
        start_from: Option<GlobalSliceIndex>,
        end_at: Option<GlobalSliceIndex>,
    ) -> Option<BlockPosition> {
        self.find_from_top(pos, start_from, end_at, |above, below| {
            above.block_type().is_air() && !below.block_type().is_air()
        })
    }

    /// Filter is passed (above block, below block)
    fn find_from_top(
        &self,
        pos: SliceBlock,
        start_from: Option<GlobalSliceIndex>,
        end_at: Option<GlobalSliceIndex>,
        mut filter: impl FnMut(Block<C>, Block<C>) -> bool,
    ) -> Option<BlockPosition> {
        let start_from = start_from.unwrap_or_else(GlobalSliceIndex::top);
        let end_at = end_at.unwrap_or_else(GlobalSliceIndex::bottom);

        // -1 because iterating in windows of 2
        let end_at = GlobalSliceIndex::new(end_at.slice().saturating_sub(1));
        self.slices_from_top_offset()
            .skip_while(|(s, _)| *s > start_from)
            .take_while(|(s, _)| *s >= end_at)
            .tuple_windows()
            .find(|((_, above), (_, below))| filter(above[pos], below[pos]))
            .map(|((z, _), _)| pos.to_block_position(z))
    }

    // TODO set_block trait to reuse in ChunkBuilder (#46)
}

mod pair_walking {
    //! Helpers for cross_chunk_pairs_*

    use crate::occlusion::OcclusionOpacity;
    use unit::world::SlabIndex;

    use super::*;

    #[derive(Copy, Clone)]
    pub enum WhichChunk {
        ThisChunk,
        OtherChunk,
    }

    pub fn yield_corner<F: FnMut(WhichChunk, BlockPosition, NeighbourOpacity), C: WorldContext>(
        which_chunk: WhichChunk,
        lower_slice_above: Option<Slice<C>>,
        lower_slab: SlabIndex,
        lower_slice: LocalSliceIndex,
        upper: Slice<C>,
        direction: NeighbourOffset,
        f: &mut F,
    ) {
        debug_assert!(!direction.is_aligned());

        let corner_pos = |direction| -> (BlockCoord, BlockCoord) {
            match direction {
                NeighbourOffset::SouthEast => (CHUNK_SIZE.as_block_coord() - 1, 0),
                NeighbourOffset::NorthEast => (
                    CHUNK_SIZE.as_block_coord() - 1,
                    CHUNK_SIZE.as_block_coord() - 1,
                ),
                NeighbourOffset::NorthWest => (0, CHUNK_SIZE.as_block_coord() - 1),
                NeighbourOffset::SouthWest => (0, 0),
                _ => unsafe { unreachable_unchecked() },
            }
        };

        let this_pos = corner_pos(direction);
        if let Some(lower_slice_above) = lower_slice_above {
            // dont bother if block directly above is solid
            if lower_slice_above[this_pos].opacity().solid() {
                return;
            }
        }

        // just check single block
        let other_pos = corner_pos(direction.opposite());
        let opacity = upper[other_pos].opacity();

        if opacity.solid() {
            let mut opacities = NeighbourOpacity::default();
            opacities[direction as usize] = OcclusionOpacity::Known(opacity);

            // get block pos in this chunk
            let block_pos = {
                let slice_idx = lower_slice.to_global(lower_slab);
                BlockPosition::new_unchecked(this_pos.0, this_pos.1, slice_idx)
            };

            f(which_chunk, block_pos, opacities);
        }
    }

    pub fn calculate_boundary_slice_block_offsets(
        direction: NeighbourOffset,
        coords: &mut [(BlockCoord, BlockCoord); CHUNK_SIZE.as_usize()],
    ) {
        match direction {
            NeighbourOffset::North => {
                let xs = (0..CHUNK_SIZE.as_block_coord()).rev(); // reverse to iterate anti clockwise
                let ys = repeat(CHUNK_SIZE.as_block_coord() - 1);
                xs.zip(ys).enumerate().for_each(|(i, c)| coords[i] = c);
            }
            NeighbourOffset::South => {
                let xs = 0..CHUNK_SIZE.as_block_coord();
                let ys = repeat(0);
                xs.zip(ys).enumerate().for_each(|(i, c)| coords[i] = c);
            }
            NeighbourOffset::West => {
                let xs = repeat(0);
                let ys = (0..CHUNK_SIZE.as_block_coord()).rev(); // reverse to iterate anti clockwise
                xs.zip(ys).enumerate().for_each(|(i, c)| coords[i] = c);
            }
            NeighbourOffset::East => {
                let xs = repeat(CHUNK_SIZE.as_block_coord() - 1);
                let ys = 0..CHUNK_SIZE.as_block_coord();
                xs.zip(ys).enumerate().for_each(|(i, c)| coords[i] = c);
            }
            _ => unreachable!(),
        }
    }

    pub fn extend_boundary_slice_block(
        direction: NeighbourOffset,
        (x, y): (BlockCoord, BlockCoord),
    ) -> (BlockCoord, BlockCoord) {
        match direction {
            NeighbourOffset::North => (x, 0),
            NeighbourOffset::South => (x, CHUNK_SIZE.as_block_coord() - 1),
            NeighbourOffset::West => (CHUNK_SIZE.as_block_coord() - 1, y),
            NeighbourOffset::East => (0, y),
            _ => unreachable!(),
        }
    }

    pub fn yield_side<F: FnMut(WhichChunk, BlockPosition, NeighbourOpacity), C: WorldContext>(
        which_chunk: WhichChunk,
        lower_slice_above: Option<Slice<C>>,
        lower_slab: SlabIndex,
        lower_slice: LocalSliceIndex,
        upper: Slice<C>,
        direction: NeighbourOffset,
        f: &mut F,
    ) {
        debug_assert!(direction.is_aligned());

        let mut coord_range = [(0, 0); CHUNK_SIZE.as_usize()];
        calculate_boundary_slice_block_offsets(direction, &mut coord_range);

        // None, Some(0,0), Some(1,0), ... None
        let adjacent_coords = coord_range.iter().copied();
        for (coord, (left, centre, right)) in adjacent_coords.clone().zip(
            once(None)
                .chain(
                    adjacent_coords
                        .map(|(x, y)| Some(extend_boundary_slice_block(direction, (x, y)))),
                )
                .chain(once(None))
                .tuple_windows(),
        ) {
            if let Some(lower_slice_above) = &lower_slice_above {
                // dont bother if block directly above is solid
                if lower_slice_above[coord].opacity().solid() {
                    continue;
                }
            }

            let mut opacities = NeighbourOpacity::default();

            // directly across is certainly present
            opacities[direction as usize] =
                OcclusionOpacity::Known(upper[centre.unwrap()].opacity());

            if let Some(left) = left {
                opacities[direction.next() as usize] =
                    OcclusionOpacity::Known(upper[left].opacity());
            }

            if let Some(right) = right {
                opacities[direction.prev() as usize] =
                    OcclusionOpacity::Known(upper[right].opacity());
            }

            // get block pos in this chunk
            let block_pos = {
                let slice_idx = lower_slice.to_global(lower_slab);
                BlockPosition::new_unchecked(coord.0, coord.1, slice_idx)
            };

            f(which_chunk, block_pos, opacities)
        }
    }
}

impl<C: WorldContext> Default for RawChunkTerrain<C> {
    /// Has a single empty placeholder slab at index 0
    fn default() -> Self {
        let mut terrain = Self {
            slabs: DoubleSidedVec::with_capacity(8),
        };

        terrain.slabs.add(Slab::empty_placeholder(), 0);

        terrain
    }
}

#[cfg(test)]
mod tests {
    use unit::world::CHUNK_SIZE;
    use unit::world::{GlobalSliceIndex, WorldPositionRange, SLAB_SIZE};

    use crate::chunk::slab::Slab;
    use crate::chunk::terrain::BaseTerrain;
    use crate::chunk::ChunkBuilder;
    use crate::occlusion::{OcclusionFace, VertexOcclusion};
    use crate::world::helpers::{
        apply_updates, load_single_chunk, world_from_chunks_blocking, DummyWorldContext,
    };
    use crate::{World, WorldArea, WorldRef};

    use super::*;
    use crate::helpers::{loader_from_chunks_blocking, DummyBlockType};
    use crate::loader::WorldTerrainUpdate;
    use crate::navigation::discovery::AreaDiscovery;
    use std::convert::TryInto;

    #[test]
    fn empty() {
        let terrain = RawChunkTerrain::<DummyWorldContext>::default();
        assert_eq!(terrain.slab_count(), 1);
    }

    #[test]
    #[should_panic]
    fn no_dupes() {
        let mut terrain = RawChunkTerrain::<DummyWorldContext>::default();
        terrain.add_empty_placeholder_slab(0);
    }

    #[test]
    fn slabs() {
        let mut terrain = RawChunkTerrain::<DummyWorldContext>::default();

        terrain.add_empty_placeholder_slab(1);
        terrain.add_empty_placeholder_slab(2);

        terrain.add_empty_placeholder_slab(-1);
        terrain.add_empty_placeholder_slab(-2);

        let slabs: Vec<i32> = terrain
            .slabs_from_top()
            .map(|(_, index)| index.as_i32())
            .collect();
        assert_eq!(slabs, vec![2, 1, 0, -1, -2]);

        let slabs: Vec<i32> = terrain
            .slabs_from_bottom()
            .map(|(_, index)| index.as_i32())
            .collect();
        assert_eq!(slabs, vec![-2, -1, 0, 1, 2]);
    }

    #[test]
    fn slab_index() {
        assert_eq!(GlobalSliceIndex::new(4).slab_index(), SlabIndex(0));
        assert_eq!(GlobalSliceIndex::new(0).slab_index(), SlabIndex(0));
        assert_eq!(GlobalSliceIndex::new(-3).slab_index(), SlabIndex(-1));
        assert_eq!(GlobalSliceIndex::new(-20).slab_index(), SlabIndex(-1));
        assert_eq!(GlobalSliceIndex::new(100).slab_index(), SlabIndex(3));
    }

    #[test]
    fn block_views() {
        let mut terrain = RawChunkTerrain::<DummyWorldContext>::default();

        *terrain.slice_mut(0).unwrap()[(0, 0)].block_type_mut() = DummyBlockType::Stone;
        assert_eq!(
            terrain.slice(GlobalSliceIndex::new(0)).unwrap()[(0, 0)].block_type(),
            DummyBlockType::Stone
        );
        assert_eq!(
            terrain.slice(10).unwrap()[(0, 0)].block_type(),
            DummyBlockType::Air
        );

        assert!(terrain.slice(SLAB_SIZE.as_i32()).is_none());
        assert!(terrain.slice(-1).is_none());

        terrain.add_empty_placeholder_slab(-1);
        *terrain.slice_mut(-1).unwrap()[(3, 3)].block_type_mut() = DummyBlockType::Grass;
        assert_eq!(
            terrain.slice(-1).unwrap()[(3, 3)].block_type(),
            DummyBlockType::Grass
        );
        assert_eq!(
            terrain.get_block_tup((3, 3, -1)).unwrap().block_type(),
            DummyBlockType::Grass
        );

        let mut terrain = RawChunkTerrain::<DummyWorldContext>::default();
        assert_eq!(
            terrain.set_block(
                (2, 0, 0).try_into().unwrap(),
                DummyBlockType::Stone,
                SlabCreationPolicy::PleaseDont
            ),
            true
        );
        assert_eq!(
            terrain.set_block(
                (2, 0, -2).try_into().unwrap(),
                DummyBlockType::Stone,
                SlabCreationPolicy::PleaseDont
            ),
            false
        );
        let mut blocks = Vec::new();
        terrain.blocks(&mut blocks);

        assert_eq!(blocks[0].0, (0, 0, 0).try_into().unwrap());
        assert_eq!(blocks[1].0, (1, 0, 0).try_into().unwrap());
        assert_eq!(
            blocks
                .iter()
                .filter(|(_, b)| b.block_type() == DummyBlockType::Stone)
                .count(),
            1
        );
    }

    #[test]
    fn slab_areas() {
        // slab with flat slice 0 should have 1 area
        let mut slab = Slab::<DummyWorldContext>::empty();
        slab.slice_mut(LocalSliceIndex::new_unchecked(0))
            .fill(DummyBlockType::Stone);

        let area_count = AreaDiscovery::from_slab(&slab, SlabIndex(0), None).flood_fill_areas();
        assert_eq!(area_count, 1);

        // slab with 2 unconnected floors should have 2
        let mut slab = Slab::<DummyWorldContext>::empty();
        slab.slice_mut(LocalSliceIndex::new_unchecked(0))
            .fill(DummyBlockType::Stone);
        slab.slice_mut(LocalSliceIndex::new_unchecked(5))
            .fill(DummyBlockType::Stone);

        let area_count = AreaDiscovery::from_slab(&slab, SlabIndex(0), None).flood_fill_areas();
        assert_eq!(area_count, 2);
    }

    //noinspection DuplicatedCode
    #[test]
    fn slab_areas_jump() {
        // terrain with accessible jumps should still be 1 area

        let mut terrain = ChunkBuilder::default().set_block((2, 2, 2), DummyBlockType::Stone); // solid walkable

        // full jump staircase next to it
        terrain = terrain
            .set_block((3, 2, 3), DummyBlockType::Stone)
            .set_block((4, 2, 4), DummyBlockType::Stone)
            .set_block((5, 2, 4), DummyBlockType::Stone);

        // 1 area still
        let chunk = load_single_chunk(terrain);
        assert_eq!(chunk.areas().count(), 1);

        // too big jump out of reach is still unreachable
        let terrain = ChunkBuilder::default()
            .set_block((2, 2, 2), DummyBlockType::Stone)
            .set_block((3, 2, 3), DummyBlockType::Stone)
            .set_block((4, 2, 7), DummyBlockType::Stone);

        let chunk = load_single_chunk(terrain);
        assert_eq!(chunk.areas().count(), 2);

        // if above is blocked, can't jump
        let terrain = ChunkBuilder::default()
            .set_block((2, 2, 2), DummyBlockType::Stone)
            .set_block((3, 2, 3), DummyBlockType::Stone)
            .set_block((2, 2, 4), DummyBlockType::Stone); // blocks jump!

        // so 2 areas expected
        let chunk = load_single_chunk(terrain);
        assert_eq!(chunk.areas().count(), 2);
    }

    #[test]
    fn cross_slab_walkability() {
        // a slab whose top layer is solid should mean the slab above's z=0 is walkable

        let terrain = ChunkBuilder::default()
            .set_block((0, 0, SLAB_SIZE.as_i32()), DummyBlockType::Air) // add upper slab
            .fill_slice(SLAB_SIZE.as_i32() - 1, DummyBlockType::Stone); // fill top layer of first slab

        let terrain = load_single_chunk(terrain);

        // TODO 1 area at z=0
        assert_eq!(terrain.areas().count(), 1);
    }

    #[test]
    fn create_slab() {
        // setting blocks in non-existent places should create a slab to fill it

        const SLAB_SIZE_I32: i32 = SLAB_SIZE.as_i32();
        let mut terrain = RawChunkTerrain::<DummyWorldContext>::default();

        // 1 slab below should not yet exist
        assert!(!terrain.set_block(
            (0, 0, -5).try_into().unwrap(),
            DummyBlockType::Stone,
            SlabCreationPolicy::PleaseDont
        ));
        assert!(terrain.get_block_tup((0, 0, -5)).is_none());
        assert_eq!(terrain.slab_count(), 1);
        assert_eq!(
            terrain.slice_bounds_as_slabs(),
            SliceRange::from_bounds_unchecked(0, SLAB_SIZE_I32)
        );

        // now really set
        assert!(terrain.set_block(
            (0, 0, -5).try_into().unwrap(),
            DummyBlockType::Stone,
            SlabCreationPolicy::CreateAll
        ));
        assert_eq!(
            terrain
                .get_block_tup((0, 0, -5))
                .map(|b| b.block_type())
                .unwrap(),
            DummyBlockType::Stone
        );
        assert_eq!(terrain.slab_count(), 2);
        assert_eq!(
            terrain.slice_bounds_as_slabs(),
            SliceRange::from_bounds_unchecked(-SLAB_SIZE_I32, SLAB_SIZE_I32)
        );

        // set a high block that will fill the rest in with air
        assert!(terrain.set_block(
            (0, 0, 100).try_into().unwrap(),
            DummyBlockType::Grass,
            SlabCreationPolicy::CreateAll
        ));
        assert_eq!(
            terrain
                .get_block_tup((0, 0, 100))
                .map(|b| b.block_type())
                .unwrap(),
            DummyBlockType::Grass
        );
        assert_eq!(terrain.slab_count(), 5);
        assert!(terrain.slice_bounds_as_slabs().contains(100));

        for z in 0..100 {
            // air inbetween
            assert_eq!(
                terrain
                    .get_block_tup((0, 0, z))
                    .map(|b| b.block_type())
                    .unwrap(),
                DummyBlockType::Air
            );
        }
    }

    #[test]
    fn block_graph_high_jump() {
        // there should be no edge that is a jump of > 1.0

        let terrain = ChunkBuilder::new()
            .set_block((2, 2, 2), DummyBlockType::Stone)
            // technically a vertical neighbour but the jump is too high
            .set_block((3, 2, 4), DummyBlockType::Stone);

        let chunk = load_single_chunk(terrain);
        assert_eq!(chunk.areas().count(), 2); // 2 disconnected areas
    }

    //noinspection DuplicatedCode
    #[test]
    fn discovery_block_graph() {
        let terrain = ChunkBuilder::new()
            .fill_slice(51, DummyBlockType::Stone)
            .set_block((2, 2, 52), DummyBlockType::Grass);

        let chunk = load_single_chunk(terrain);

        let graph = chunk
            .block_graph_for_area(WorldArea::new_with_slab((0, 0), SlabIndex(1)))
            .unwrap();

        // 4 flat connections
        assert_eq!(
            graph.edges((5, 5, 52).try_into().unwrap()),
            vec![
                ((4, 5, 52).try_into().unwrap(), EdgeCost::Walk),
                ((5, 4, 52).try_into().unwrap(), EdgeCost::Walk),
                ((5, 6, 52).try_into().unwrap(), EdgeCost::Walk),
                ((6, 5, 52).try_into().unwrap(), EdgeCost::Walk),
            ]
        );

        // step up on 1 side
        assert_eq!(
            graph.edges((2, 3, 52).try_into().unwrap()),
            vec![
                ((1, 3, 52).try_into().unwrap(), EdgeCost::Walk),
                ((2, 2, 53).try_into().unwrap(), EdgeCost::JumpUp),
                ((2, 4, 52).try_into().unwrap(), EdgeCost::Walk),
                ((3, 3, 52).try_into().unwrap(), EdgeCost::Walk),
            ]
        );

        // step down on all sides
        assert_eq!(
            graph.edges((2, 2, 53).try_into().unwrap()),
            vec![
                ((1, 2, 52).try_into().unwrap(), EdgeCost::JumpDown),
                ((2, 1, 52).try_into().unwrap(), EdgeCost::JumpDown),
                ((2, 3, 52).try_into().unwrap(), EdgeCost::JumpDown),
                ((3, 2, 52).try_into().unwrap(), EdgeCost::JumpDown),
            ]
        );
    }

    #[test]
    fn slice_index_in_slab() {
        // positives are simple modulus
        assert_eq!(
            GlobalSliceIndex::new(5).to_local(),
            LocalSliceIndex::new_unchecked(5)
        );
        assert_eq!(
            GlobalSliceIndex::new(SLAB_SIZE.as_i32() + 4).to_local(),
            LocalSliceIndex::new_unchecked(4)
        );

        // negatives work backwards
        assert_eq!(GlobalSliceIndex::new(-1).to_local(), LocalSliceIndex::top());
    }

    #[test]
    fn slice_index_in_chunk() {
        assert_eq!(
            LocalSliceIndex::new_unchecked(5).to_global(SlabIndex(0)),
            GlobalSliceIndex::new(5)
        );
        assert_eq!(
            LocalSliceIndex::new_unchecked(5).to_global(SlabIndex(1)),
            GlobalSliceIndex::new(SLAB_SIZE.as_i32() + 5),
        );

        assert_eq!(
            LocalSliceIndex::new_unchecked(0).to_global(SlabIndex(-1)),
            GlobalSliceIndex::new(-SLAB_SIZE.as_i32()),
        );
        assert_eq!(
            LocalSliceIndex::new_unchecked(1).to_global(SlabIndex(-1)),
            GlobalSliceIndex::new(-SLAB_SIZE.as_i32() + 1),
        );
    }

    #[test]
    fn occlusion_in_slab() {
        // no occlusion because the block directly above 2,2,2 is solid
        let mut terrain = ChunkBuilder::default()
            .set_block((2, 2, 2), DummyBlockType::Dirt)
            .set_block((2, 2, 3), DummyBlockType::Stone)
            .set_block((2, 3, 3), DummyBlockType::Dirt);

        let chunk = load_single_chunk(terrain.deep_clone());

        let (occlusion, _) = chunk
            .get_block_tup((2, 2, 2))
            .unwrap()
            .occlusion()
            .resolve_vertices(OcclusionFace::Top);

        assert_eq!(
            occlusion,
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll
            ]
        );

        // occlusion will be populated if block directly above it is air
        terrain = terrain.set_block((2, 2, 3), DummyBlockType::Air);
        let chunk = load_single_chunk(terrain);

        let (occlusion, _) = chunk
            .get_block_tup((2, 2, 2))
            .unwrap()
            .occlusion()
            .resolve_vertices(OcclusionFace::Top);

        assert_eq!(
            occlusion,
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly
            ]
        );
    }

    #[test]
    fn occlusion_across_slab() {
        logging::for_tests();
        let terrain = ChunkBuilder::default()
            .set_block((2, 2, SLAB_SIZE.as_i32() - 1), DummyBlockType::Dirt)
            .set_block(
                (2, 3, SLAB_SIZE.as_i32()), // next slab up
                DummyBlockType::Dirt,
            );

        let terrain = load_single_chunk(terrain);

        let (occlusion, _) = terrain
            .get_block_tup((2, 2, SLAB_SIZE.as_i32() - 1))
            .unwrap()
            .occlusion()
            .resolve_vertices(OcclusionFace::Top);

        assert_eq!(
            occlusion,
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly
            ]
        );
    }

    fn occlusion(
        world: &World<DummyWorldContext>,
        chunk: ChunkLocation,
        block: (i32, i32, i32),
        face: OcclusionFace,
    ) -> [VertexOcclusion; 4] {
        let chunk = world.find_chunk_with_pos(chunk).unwrap();
        let block = chunk.get_block(block.try_into().unwrap()).unwrap();
        let (occlusion, _) = block.occlusion().resolve_vertices(face);
        occlusion
    }

    #[test]
    fn occlusion_across_chunk_sides() {
        // logging::for_tests();

        let a = ChunkBuilder::new()
            .set_block((0, 0, SLAB_SIZE.as_i32()), DummyBlockType::Grass) // slab 1
            .set_block((0, 0, 0), DummyBlockType::Grass) // slab 0
            .build((0, 0));

        let b = ChunkBuilder::new()
            // occludes 0,0,0 in slab 0
            .set_block((CHUNK_SIZE.as_i32() - 1, 0, 1), DummyBlockType::Stone)
            .build((-1, 0));

        // occludes 0,0,0 in slab 0
        let c = ChunkBuilder::new()
            .set_block((0, CHUNK_SIZE.as_i32() - 1, 1), DummyBlockType::Stone)
            .build((0, -1));

        let world = world_from_chunks_blocking(vec![a, b, c]).into_inner();

        // 0,0,0 occluded by 2 chunk neighbours
        assert!(world.block((-1, 0, 1)).unwrap().opacity().solid());
        assert!(world.block((0, -1, 1)).unwrap().opacity().solid());
        assert!(matches!(
            occlusion(&world, ChunkLocation(0, 0), (0, 0, 0), OcclusionFace::Top),
            [
                VertexOcclusion::Full,
                VertexOcclusion::Mildly,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly
            ]
        ));
    }

    #[test]
    fn lazy_occlusion_top_only() {
        fn mk_chunks(block_off: bool) -> WorldRef<DummyWorldContext> {
            let a = ChunkBuilder::new()
                .set_block((CHUNK_SIZE.as_i32() - 1, 0, 0), DummyBlockType::Grass)
                .build((-1, 0));

            let blockage = if block_off {
                DummyBlockType::Grass
            } else {
                DummyBlockType::Air
            };

            let b = ChunkBuilder::new()
                .set_block((0, 0, -1), DummyBlockType::Stone)
                .set_block((1, 0, 0), DummyBlockType::Stone)
                .set_block((0, 0, 0), blockage)
                .build((0, 0));

            world_from_chunks_blocking(vec![a, b])
        }

        let world_ref = mk_chunks(false);
        let world = world_ref.borrow();

        // 0, 0, -1 occluded by (-1, 0, 0) in other chunk and (1, 0, 0) in other slab
        assert!(world.block((0, 0, -1)).unwrap().opacity().solid());
        assert!(world.block((-1, 0, 0)).unwrap().opacity().solid());
        assert!(world.block((0, 0, 0)).unwrap().opacity().transparent());
        assert_eq!(
            occlusion(&world, ChunkLocation(0, 0), (0, 0, -1), OcclusionFace::Top),
            [
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly
            ]
        );

        // ... but when (0,0,0) is solid, (0,0,-1) is hidden so it shouldnt be updated
        let world_ref = mk_chunks(true);
        let world = world_ref.borrow();

        assert!(world.block((0, 0, 0)).unwrap().opacity().solid());
        assert_eq!(
            occlusion(&world, ChunkLocation(0, 0), (0, 0, -1), OcclusionFace::Top),
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll
            ]
        );
    }

    #[test]
    fn occlusion_across_chunk_corner() {
        let a = ChunkBuilder::new()
            // 0, 15, 0
            .set_block((0, CHUNK_SIZE.as_i32() - 1, 0), DummyBlockType::Stone)
            .build((0, 0));

        let b = ChunkBuilder::new()
            // -1, 16, 1, occludes 0,0,0
            .set_block((CHUNK_SIZE.as_i32() - 1, 0, 1), DummyBlockType::Grass)
            .build((-1, 1));

        let c = ChunkBuilder::new()
            // 0, 16, 1, occludes 0,0,0
            .set_block((0, 0, 1), DummyBlockType::Grass)
            .build((0, 1));

        let world = world_from_chunks_blocking(vec![a, b, c]).into_inner();

        // 0,0,0 occluded by corner on 2 sides
        assert!(world
            .block((0, CHUNK_SIZE.as_i32() - 1, 0))
            .unwrap()
            .opacity()
            .solid());
        assert!(world
            .block((-1, CHUNK_SIZE.as_i32(), 1))
            .unwrap()
            .opacity()
            .solid());
        assert!(world
            .block((0, CHUNK_SIZE.as_i32(), 1))
            .unwrap()
            .opacity()
            .solid());
        assert!(matches!(
            occlusion(
                &world,
                ChunkLocation(0, 0),
                (0, CHUNK_SIZE.as_i32() - 1, 0),
                OcclusionFace::Top
            ),
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mostly,
            ]
        ));
    }

    #[test]
    fn cloned_slab_cow_is_updated() {
        let mut terrain = RawChunkTerrain::<DummyWorldContext>::default();
        let old = terrain.slabs.get(0).unwrap().clone(); // reference to force a cow clone

        let slab = terrain.slab_mut(SlabIndex(0)).unwrap();
        let slice_0 = LocalSliceIndex::new_unchecked(0);
        slab.slice_mut(slice_0)
            .set_block((0, 0), DummyBlockType::Stone);

        // old reference is "dangling", pointing to old
        let immut = terrain.slab(SlabIndex(0)).unwrap();
        assert_eq!(old.slice(slice_0)[(0, 0)].block_type(), DummyBlockType::Air);
        assert_eq!(
            immut.slice(slice_0)[(0, 0)].block_type(),
            DummyBlockType::Stone
        );
    }

    #[test]
    fn area_discovery_after_modification() {
        // regression test for bug where area discovery was only looking for jump ups and not down

        let mut loader = loader_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_range((0, 0, 0), (2, 2, 1), |_| DummyBlockType::Stone)
            .build((0, 0))]);

        let world_ref = loader.world();
        let assert_single_area = || {
            let w = world_ref.borrow();

            // just the one area
            assert_eq!(
                w.find_chunk_with_pos(ChunkLocation(0, 0))
                    .unwrap()
                    .areas()
                    .count(),
                1
            );
        };

        assert_single_area();

        // dig out a block
        let updates = [WorldTerrainUpdate::new(
            WorldPositionRange::with_single((0, 0, 1)),
            DummyBlockType::Air,
        )];
        apply_updates(&mut loader, &updates).expect("updates failed");

        assert_single_area();

        // dig out another block
        let updates = [WorldTerrainUpdate::new(
            WorldPositionRange::with_single((1, 1, 1)),
            DummyBlockType::Air,
        )];
        apply_updates(&mut loader, &updates).expect("updates failed");

        assert_single_area();
    }

    #[test]
    fn stale_areas_removed_after_modification() {
        let mut loader = loader_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_range((0, 0, 0), (5, 0, 0), |_| DummyBlockType::Stone) // 5x1x1 ground
            .fill_range((3, 0, 0), (3, 0, 5), |_| DummyBlockType::Stone) // 1x1xz wall cutting it in half
            .build((0, 0))]);

        let world_ref = loader.world();

        let get_areas = || {
            let w = world_ref.borrow();
            w.find_chunk_with_pos(ChunkLocation(0, 0))
                .unwrap()
                .areas()
                .copied()
                .collect_vec()
        };

        // 3 areas initially, 1 on either side of the wall and 1 atop
        assert_eq!(get_areas().len(), 3);

        // remove some of the wall to combine the 2 areas
        let updates = [WorldTerrainUpdate::new(
            WorldPositionRange::with_inclusive_range((3, 0, 1), (3, 0, 2)),
            DummyBlockType::Air,
        )];
        apply_updates(&mut loader, &updates).expect("updates failed");

        // now only 2 areas, the ground and atop the wall
        assert_eq!(get_areas().len(), 2);

        // remove the wall completely
        let updates = [WorldTerrainUpdate::new(
            WorldPositionRange::with_inclusive_range((3, 0, 1), (3, 0, 10)),
            DummyBlockType::Air,
        )];
        apply_updates(&mut loader, &updates).expect("updates failed");

        // now only 1 area, the ground
        assert_eq!(get_areas().len(), 1);

        // remove the ground too
        let updates = [WorldTerrainUpdate::new(
            WorldPositionRange::with_inclusive_range((0, 0, 0), (5, 5, 5)),
            DummyBlockType::Air,
        )];
        apply_updates(&mut loader, &updates).expect("updates failed");

        // now the chunk is empty
        assert_eq!(get_areas().len(), 0);
    }
}
