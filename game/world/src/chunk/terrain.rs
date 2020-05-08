use std::collections::HashMap;
use std::hint::unreachable_unchecked;
use std::iter::{once, repeat};
use std::ops::{Deref, DerefMut};

use common::*;
pub(crate) use pair_walking::WhichChunk;
use unit::dim::CHUNK_SIZE;
use unit::world::{BlockCoord, BlockPosition, ChunkPosition, SliceIndex};

use crate::area::discovery::AreaDiscovery;
use crate::area::{BlockGraph, ChunkArea, WorldArea};
use crate::block::{Block, BlockType};
use crate::chunk::double_sided_vec::DoubleSidedVec;
use crate::chunk::slab::{Slab, SlabIndex, SLAB_SIZE};
use crate::chunk::slice::{unflatten_index, Slice, SliceMut};
use crate::occlusion::{BlockOcclusion, NeighbourOffset, NeighbourOpacity};
use crate::{EdgeCost, SliceRange};

pub(crate) type SlabPointer = Box<Slab>;

// TODO expensive to clone, use Cow if actually necessary
/// Terrain only
#[derive(Clone)]
pub struct RawChunkTerrain {
    slabs: DoubleSidedVec<SlabPointer>,
}

#[cfg_attr(test, derive(Clone))]
pub struct ChunkTerrain {
    raw_terrain: RawChunkTerrain,
    areas: HashMap<WorldArea, BlockGraph>,
}

pub trait BaseTerrain {
    fn raw_terrain(&self) -> &RawChunkTerrain;
    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain;

    fn slice<S: Into<SliceIndex>>(&self, index: S) -> Option<Slice> {
        let index = index.into();
        let slab_idx = RawChunkTerrain::slab_index_for_slice(index);
        self.raw_terrain()
            .slabs
            .get(slab_idx)
            .map(|ptr| ptr.slice(RawChunkTerrain::slice_index_in_slab(index)))
    }

    fn slice_mut<S: Into<SliceIndex>>(&mut self, index: S) -> Option<SliceMut> {
        let index = index.into();
        let slab_idx = RawChunkTerrain::slab_index_for_slice(index);
        self.raw_terrain_mut()
            .slabs
            .get_mut(slab_idx)
            .map(|ptr| ptr.slice_mut(RawChunkTerrain::slice_index_in_slab(index)))
    }

    fn get_block<B: Into<BlockPosition>>(&self, pos: B) -> Option<Block> {
        let pos = pos.into();
        self.slice(pos.2).map(|slice| slice[pos])
    }

    fn get_block_type<B: Into<BlockPosition>>(&self, pos: B) -> Option<BlockType> {
        self.get_block(pos).map(|b| b.block_type())
    }

    /// Returns the range of slices in this terrain rounded to the nearest slab
    fn slice_bounds_as_slabs(&self) -> SliceRange {
        let mut slabs = self.raw_terrain().slabs.indices_increasing();
        let bottom = slabs.next().unwrap_or(0);
        let top = slabs.last().unwrap_or(0) + 1;

        SliceRange::from_bounds(bottom * SLAB_SIZE.as_i32(), top * SLAB_SIZE.as_i32())
    }

    /// Only for tests
    #[cfg(test)]
    fn blocks<'a>(
        &self,
        out: &'a mut Vec<(BlockPosition, Block)>,
    ) -> &'a mut Vec<(BlockPosition, Block)> {
        use crate::chunk::BLOCK_COUNT_SLICE;

        let bottom_slab = self.raw_terrain().slabs_from_bottom().next().unwrap();

        let low_z = bottom_slab.index() * SLAB_SIZE.as_i32();
        let high_z = low_z + (self.raw_terrain().slab_count() * SLAB_SIZE.as_usize()) as i32;

        let total_size: usize = ((high_z - low_z) * BLOCK_COUNT_SLICE as i32) as usize;
        out.reserve(total_size);
        out.clear();

        let iter_from = if low_z != 0 { low_z + 1 } else { low_z };

        for z in iter_from..high_z {
            for y in 0..CHUNK_SIZE.as_block_coord() {
                for x in 0..CHUNK_SIZE.as_block_coord() {
                    let pos: BlockPosition = (x, y, z).into();
                    let block = self.get_block(pos);
                    out.push((pos, block.unwrap()));
                }
            }
        }

        out
    }
}

impl BaseTerrain for RawChunkTerrain {
    fn raw_terrain(&self) -> &RawChunkTerrain {
        self
    }

    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain {
        self
    }
}

#[derive(Copy, Clone)]
pub enum SlabCreationPolicy {
    /// Don't add missing slabs
    PleaseDont,

    /// Create the missing slab and all intermediate slabs
    CreateAll,
}

impl RawChunkTerrain {
    fn slabs_from_top(&self) -> impl Iterator<Item = &Slab> {
        self.slabs.iter_decreasing().map(|ptr| ptr.deref())
    }

    pub(crate) fn slabs_from_bottom(&self) -> impl Iterator<Item = &Slab> {
        self.slabs.iter_increasing().map(|ptr| ptr.deref())
    }

    pub(crate) fn slabs_from_bottom_mut(&mut self) -> impl Iterator<Item = (SlabIndex, &mut Slab)> {
        self.slabs
            .iter_mut_increasing()
            .map(|ptr| ptr.deref_mut())
            .map(|slab| (slab.index(), slab))
    }

    pub(crate) fn slabs_from_top_mut(&mut self) -> impl Iterator<Item = (SlabIndex, &mut Slab)> {
        self.slabs
            .iter_mut_decreasing()
            .map(|ptr| ptr.deref_mut())
            .map(|slab| (slab.index(), slab))
    }

    fn add_slab(&mut self, slab: SlabPointer) {
        let idx = slab.index();
        self.slabs.add(slab, idx);
    }

    pub(crate) fn slab(&self, index: SlabIndex) -> Option<&Slab> {
        self.slabs.get(index).map(|s| s.deref())
    }

    /// Creates slabs up to and including target
    fn create_slabs_until(&mut self, target: SlabIndex) {
        self.slabs
            .fill_until(target, |idx| SlabPointer::new(Slab::empty(idx)));
    }

    pub(crate) fn slab_index_for_slice(slice: SliceIndex) -> SlabIndex {
        (slice.0 as f32 / SLAB_SIZE.as_f32()).floor() as SlabIndex
    }

    pub(crate) fn slice_index_in_slab(slice: SliceIndex) -> SliceIndex {
        let SliceIndex(mut idx) = slice;
        idx %= SLAB_SIZE.as_i32(); // cap at slab size
        if idx.is_negative() {
            // negative slices flip
            idx += SLAB_SIZE.as_i32();
        }
        SliceIndex(idx)
    }

    pub fn slab_count(&self) -> usize {
        self.slabs.len()
    }

    /// Returns the range of slices in this terrain rounded to the nearest slab
    pub fn slice_bounds_as_slabs(&self) -> SliceRange {
        let mut slabs = self.slabs.indices_increasing();
        let bottom = slabs.next().unwrap_or(0);
        let top = slabs.last().unwrap_or(0) + 1;

        SliceRange::from_bounds(bottom * SLAB_SIZE.as_i32(), top * SLAB_SIZE.as_i32())
    }

    pub fn slices_from_bottom(&self) -> impl Iterator<Item = (SliceIndex, Slice)> {
        self.slabs_from_bottom()
            .flat_map(|slab| slab.slices_from_bottom())
    }

    /// (global slice index, slice)
    pub fn slices_from_top_offset(&self) -> impl Iterator<Item = (SliceIndex, Slice)> {
        self.slabs_from_top().flat_map(|slab| {
            slab.slices_from_bottom().rev().map(move |(z, slice)| {
                (
                    RawChunkTerrain::slice_index_in_chunk(slab.index(), z),
                    slice,
                )
            })
        })
    }

    /// If slab doesn't exist, does nothing and returns false
    pub fn try_set_block<P, B>(&mut self, pos: P, block: B) -> bool
    where
        P: Into<BlockPosition>,
        B: Into<Block>,
    {
        self.set_block(pos, block, SlabCreationPolicy::PleaseDont)
    }

    /// Returns if block was set successfully, depends on slab creation policy
    pub fn set_block<P, B>(&mut self, pos: P, block: B, policy: SlabCreationPolicy) -> bool
    where
        P: Into<BlockPosition>,
        B: Into<Block>,
    {
        let pos = pos.into();
        let block = block.into();
        let mut try_again = true;

        loop {
            if let Some(mut slice) = self.slice_mut(pos.2) {
                // nice, slice exists: we're done
                slice[pos] = block;
                return true;
            }

            // slice doesn't exist

            // we tried twice and failed both times, to shame
            if !try_again {
                return false;
            }

            match policy {
                SlabCreationPolicy::PleaseDont => {
                    // oh well we tried
                    return false;
                }
                SlabCreationPolicy::CreateAll => {
                    // create slabs
                    let target_slab = Self::slab_index_for_slice(pos.2);
                    self.create_slabs_until(target_slab);

                    // try again once more
                    try_again = false;
                    continue;
                }
            };
        }
    }

    pub(crate) fn slice_index_in_chunk(slab: SlabIndex, slice: SliceIndex) -> SliceIndex {
        let z_offset = slab * SLAB_SIZE.as_i32();
        slice + z_offset
    }

    pub fn with_block_mut_unchecked<F: FnMut(&mut Block)>(&mut self, pos: BlockPosition, mut f: F) {
        let mut slice = self.slice_mut(pos.2).unwrap();
        let block = &mut slice[pos];
        f(block);
    }

    /// offset: self->other
    pub(crate) fn cross_chunk_pairs_foreach<
        F: FnMut(WhichChunk, BlockPosition, NeighbourOpacity),
    >(
        &'_ self,
        other: &'_ Self,
        offset: NeighbourOffset,
        mut f: F,
    ) {
        let offset_opposite = offset.opposite();

        let yield_ = if offset.is_aligned() {
            pair_walking::yield_side
        } else {
            pair_walking::yield_corner
        };

        // find slab range
        let (my_min, my_max) = self.slabs.index_range();
        let (ur_min, ur_max) = other.slabs.index_range();

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
            let lower_slice_above = lower.slab(lower_slab_index + 1).map(|slab| slab.slice(0));

            let (_, bottom_slice) = higher.slices_from_bottom().next().unwrap();
            yield_(
                which_lower,
                lower_slice_above,
                lower_slab_index,
                SliceIndex(SLAB_SIZE.as_i32() - 1),
                bottom_slice,
                dir,
                &mut f,
            )
        }

        // continue from the common min = max of the mins
        let first_common_slab = my_min.max(ur_min);

        // yield slices up until first max
        let first_max = my_max.min(ur_max);

        for (slab_index, next_slab_index) in (first_common_slab..=first_max)
            .map(Some)
            .chain(once(None))
            .tuple_windows()
        {
            let slab_index = slab_index.unwrap(); // always Some
            let this_slab = self.slab(slab_index).unwrap();
            let other_slab = other.slab(slab_index).unwrap();

            for z in 0..SLAB_SIZE.as_i32() - 1 {
                let this_slice_above = this_slab.slice(z + 1);
                let upper_slice = other_slab.slice(z + 1);
                yield_(
                    WhichChunk::ThisChunk,
                    Some(this_slice_above),
                    slab_index,
                    SliceIndex(z),
                    upper_slice,
                    offset,
                    &mut f,
                );

                let upper_slice = this_slab.slice(z + 1);
                let other_slice_above = other_slab.slice(z + 1);
                yield_(
                    WhichChunk::OtherChunk,
                    Some(other_slice_above),
                    slab_index,
                    SliceIndex(z),
                    upper_slice,
                    offset_opposite,
                    &mut f,
                );
            }

            // special case of top slice of one and bottom slice of next
            if let Some(next_slab_index) = next_slab_index {
                let this_slice_above = self.slab(next_slab_index).map(|slab| slab.slice(0));
                let next_slice = other.slab(next_slab_index).unwrap().slice(0);
                yield_(
                    WhichChunk::ThisChunk,
                    this_slice_above,
                    slab_index,
                    SliceIndex(SLAB_SIZE.as_i32() - 1),
                    next_slice,
                    offset,
                    &mut f,
                );

                // let top_slice = other_slab.slice(SLAB_SIZE.as_i32() - 1);
                let other_slice_above = other.slab(next_slab_index).map(|slab| slab.slice(0));
                let next_slice = self.slab(next_slab_index).unwrap().slice(0);
                yield_(
                    WhichChunk::OtherChunk,
                    other_slice_above,
                    slab_index,
                    SliceIndex(SLAB_SIZE.as_i32() - 1),
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
            let lower_slice_above = lower.slab(lower_max + 1).map(|slab| slab.slice(0));
            let bottom_slice = higher.slab(lower_max + 1).unwrap().slice(0);
            yield_(
                which_lower,
                lower_slice_above,
                lower_max,
                SliceIndex(SLAB_SIZE.as_i32() - 1),
                bottom_slice,
                dir,
                &mut f,
            );

            // no need to bother with rest of higher slabs
        }
    }

    pub(crate) fn cross_chunk_pairs_nav_foreach<
        F: FnMut(ChunkArea, ChunkArea, EdgeCost, BlockCoord, SliceIndex),
    >(
        &'_ self,
        other: &'_ Self,
        offset: NeighbourOffset,
        mut f: F,
    ) {
        for slab_idx in self.slabs.indices_increasing() {
            let my_slab = self.slabs.get(slab_idx).unwrap();

            // get loaded adjacent neighbour slab
            let ur_slab_adjacent = match other.slabs.get(slab_idx) {
                Some(s) => s,
                None => {
                    // skip this whole slab, no links to be made
                    continue;
                }
            };

            let ur_slab_below = other.slabs.get(slab_idx - 1).map(|s| s.deref());
            let ur_slab_above = other.slabs.get(slab_idx + 1).map(|s| s.deref());

            let mut coord_range = [(0, 0); CHUNK_SIZE.as_usize()];
            pair_walking::calculate_boundary_slice_block_offsets(offset, &mut coord_range);
            let x_coord_changes = match offset {
                NeighbourOffset::North | NeighbourOffset::South => true,
                _ => false,
            };

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
                            f(src, dst, cost, coord, slice_idx);

                            // done with this slice
                            // TODO could skip next slice because it cant be walkable if this one was?
                            break;
                        }
                    }
                }
            }
        }
    }

    unsafe fn slab_with_lifetime<'s>(&'_ self, idx: SlabIndex) -> Option<&'s Slab> {
        let slab = self.slabs.get(idx).map(|s| s.deref());
        std::mem::transmute(slab)
    }

    unsafe fn slab_with_lifetime_mut<'s>(&'_ mut self, idx: SlabIndex) -> Option<&'s mut Slab> {
        let slab = self.slabs.get_mut(idx).map(|s| s.deref_mut());
        std::mem::transmute(slab)
    }

    /// Returns (maybe this slab, maybe below slice, maybe above slice)
    pub(crate) fn slab_with_surrounding_slices<'a, 'b, 'c>(
        &'a mut self,
        slab_index: SlabIndex,
    ) -> (Option<&'a mut Slab>, Option<Slice<'b>>, Option<Slice<'c>>) {
        // safety: slab_index doesnt alias with slab_index-1 or slab_index+1
        let slab = unsafe { self.slab_with_lifetime_mut(slab_index) };

        if slab.is_none() {
            (None, None, None)
        } else {
            // safety: slab_index doesnt alias with slab_index-1 or slab_index+1
            unsafe {
                let below = self
                    .slab_with_lifetime(slab_index - 1)
                    .map(|s| s.slice(SLAB_SIZE.as_i32() - 1));
                let above = self.slab_with_lifetime(slab_index + 1).map(|s| s.slice(0));
                (slab, below, above)
            }
        }
    }

    // TODO set_block trait to reuse in ChunkBuilder (#46)
}

mod pair_walking {
    //! Helpers for cross_chunk_pairs_*
    use super::*;

    #[derive(Copy, Clone)]
    pub enum WhichChunk {
        ThisChunk,
        OtherChunk,
    }

    pub fn yield_corner<F: FnMut(WhichChunk, BlockPosition, NeighbourOpacity)>(
        which_chunk: WhichChunk,
        lower_slice_above: Option<Slice>,
        lower_slab: SlabIndex,
        lower_slice: SliceIndex,
        upper: Slice,
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
            opacities[direction as usize] = opacity;

            // get block pos in this chunk
            let block_pos = {
                let slice_idx = RawChunkTerrain::slice_index_in_chunk(lower_slab, lower_slice);
                BlockPosition(this_pos.0, this_pos.1, slice_idx)
            };

            f(which_chunk, block_pos, opacities);
        }
    }

    pub fn calculate_boundary_slice_block_offsets(
        direction: NeighbourOffset,
        coords: &mut [(BlockCoord, BlockCoord); CHUNK_SIZE.as_usize()],
    ) {
        debug_assert!(direction.is_aligned());

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
            _ => {
                // safety: direction is asserted to be aligned
                unsafe { unreachable_unchecked() }
            }
        }
    }

    pub fn extend_boundary_slice_block(
        direction: NeighbourOffset,
        (x, y): (BlockCoord, BlockCoord),
    ) -> (BlockCoord, BlockCoord) {
        debug_assert!(direction.is_aligned());

        match direction {
            NeighbourOffset::North => (x, 0),
            NeighbourOffset::South => (x, CHUNK_SIZE.as_block_coord() - 1),
            NeighbourOffset::West => (CHUNK_SIZE.as_block_coord() - 1, y),
            NeighbourOffset::East => (0, y),
            _ => {
                // safety: direction is asserted to be aligned
                unsafe { unreachable_unchecked() }
            }
        }
    }

    pub fn yield_side<F: FnMut(WhichChunk, BlockPosition, NeighbourOpacity)>(
        which_chunk: WhichChunk,
        lower_slice_above: Option<Slice>,
        lower_slab: SlabIndex,
        lower_slice: SliceIndex,
        upper: Slice,
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
            opacities[direction as usize] = upper[centre.unwrap()].opacity();

            if let Some(left) = left {
                opacities[direction.next() as usize] = upper[left].opacity();
            }

            if let Some(right) = right {
                opacities[direction.prev() as usize] = upper[right].opacity();
            }

            // get block pos in this chunk
            let block_pos = {
                let slice_idx = RawChunkTerrain::slice_index_in_chunk(lower_slab, lower_slice);
                BlockPosition(coord.0, coord.1, slice_idx)
            };

            f(which_chunk, block_pos, opacities)
        }
    }
}

impl Default for RawChunkTerrain {
    /// has single empty slab at index 0
    fn default() -> Self {
        let mut terrain = Self {
            slabs: DoubleSidedVec::with_capacity(8),
        };

        terrain.add_slab(SlabPointer::new(Slab::empty(0)));

        terrain
    }
}

impl ChunkTerrain {
    pub fn from_raw_terrain(mut raw_terrain: RawChunkTerrain, chunk_pos: ChunkPosition) -> Self {
        // ensure there's an empty slab at the top of each chunk, for simplified nav detection
        // TODO cow for empty slab
        raw_terrain
            .slabs
            .add_to_top(|i| SlabPointer::new(Slab::empty(i)));

        let mut terrain = Self {
            raw_terrain,
            areas: HashMap::with_capacity(32),
        };

        terrain.discover_areas(chunk_pos);
        terrain.init_occlusion();

        terrain
    }

    fn discover_areas(&mut self, chunk_pos: ChunkPosition) {
        debug!("discovering areas for chunk {:?}", chunk_pos);

        // TODO reuse a buffer for each slab

        // per slab
        for idx in self.raw_terrain.slabs.indices_increasing() {
            let (slab, slice_below, slice_above) =
                self.raw_terrain.slab_with_surrounding_slices(idx);
            let slab = slab.unwrap();

            // collect slab into local grid
            let mut discovery = AreaDiscovery::from_slab(slab, slice_below, slice_above);

            // flood fill and assign areas
            let area_count = discovery.flood_fill_areas();
            debug!("chunk {:?} slab {}: {} areas", chunk_pos, idx, area_count);

            // collect areas and graphs
            self.areas.extend(
                discovery
                    .areas_with_graph()
                    .map(|(chunk_area, block_graph)| {
                        (chunk_area.into_world_area(chunk_pos), block_graph)
                    }),
            );

            // TODO discover internal area links

            discovery.apply(slab);
        }
    }

    pub(crate) fn areas(&self) -> impl Iterator<Item = &WorldArea> {
        self.areas.keys()
    }

    pub(crate) fn block_graph_for_area(&self, area: WorldArea) -> Option<&BlockGraph> {
        self.areas.get(&area)
    }

    fn init_occlusion(&mut self) {
        self.ascending_slice_pairs_foreach(|mut slice_this, slice_next| {
            for (i, b) in slice_this
                .iter_mut()
                .enumerate()
                // this block should be solid
                .filter(|(_, b)| b.opacity().solid())
                // and the one above it should not be
                .filter(|(i, _)| (*slice_next)[*i].opacity().transparent())
            {
                let this_block = unflatten_index(i);

                // collect blocked state of each neighbour on the top face
                let mut blocked = NeighbourOpacity::default();
                for (n, offset) in NeighbourOffset::offsets() {
                    if let Some(neighbour_block) = this_block.try_add(offset) {
                        blocked[n as usize] = slice_next[neighbour_block].opacity();
                    }
                }

                *b.occlusion_mut() = BlockOcclusion::from_neighbour_opacities(blocked);
            }
        });
    }

    // TODO transmute lifetimes instead
    // (slab0 slice0 mut, slab0 slice1 immut), (slab0 slice1 mut, slab0 slice2 immut) ...
    // ... (slab0 sliceN mut, slab1 slice0), (slab1 slice0 mut, slab1 slice1) ...
    // ... (slabN sliceN-1 mut, slabN sliceN)
    pub fn ascending_slice_pairs_foreach<F: FnMut(SliceMut, Slice)>(&mut self, mut f: F) {
        // need to include a null slab at the end so the last slab is iterated too
        let indices = self
            .raw_terrain
            .slabs
            .indices_increasing()
            .map(Some)
            .chain(once(None))
            .tuple_windows();

        for (this_slab_idx, next_slab_idx) in indices {
            let this_slab_idx = this_slab_idx.unwrap(); // first slab is always Some

            let this_slab = self.raw_terrain.slabs.get_mut(this_slab_idx).unwrap();

            // exhaust this slab first
            for (this_slice_idx, next_slice_idx) in (0..Slab::slice_count()).tuple_windows() {
                let mut this_slice_mut = this_slab.slice_mut(this_slice_idx);

                // Safety: slices don't overlap and this_slice_idx != next_slice_idx
                let this_slice_mut = unsafe {
                    let ptr = this_slice_mut.as_mut_ptr();
                    SliceMut::from_ptr(ptr)
                };
                let next_slice = this_slab.slice(next_slice_idx);

                f(this_slice_mut, next_slice);
            }

            // top slice of this slab and bottom of next
            if let Some(next_slab_idx) = next_slab_idx {
                // safety: mutable and immutable slices don't overlap
                let this_slab_top_slice = unsafe {
                    // can't have a mut and immut ref to self.raw_terrain
                    let mut slice = this_slab.slice_mut(Slab::slice_count() - 1);
                    let ptr = slice.as_mut_ptr();
                    SliceMut::from_ptr(ptr)
                };

                let next_slab_bottom_slice =
                    self.raw_terrain.slabs.get(next_slab_idx).unwrap().slice(0);
                f(this_slab_top_slice, next_slab_bottom_slice);
            }
        }
    }
}

impl BaseTerrain for ChunkTerrain {
    fn raw_terrain(&self) -> &RawChunkTerrain {
        &self.raw_terrain
    }

    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain {
        &mut self.raw_terrain
    }
}

impl From<ChunkTerrain> for RawChunkTerrain {
    fn from(terrain: ChunkTerrain) -> Self {
        terrain.raw_terrain
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;

    use unit::dim::CHUNK_SIZE;
    use unit::world::SliceIndex;

    use crate::block::BlockType;
    use crate::chunk::slab::{Slab, SLAB_SIZE};
    use crate::chunk::terrain::{BaseTerrain, ChunkTerrain, SlabPointer};
    use crate::chunk::ChunkBuilder;
    use crate::occlusion::VertexOcclusion;
    use crate::world::world_from_chunks;
    use crate::World;

    use super::*;

    #[test]
    fn empty() {
        let terrain = RawChunkTerrain::default();
        assert_eq!(terrain.slab_count(), 1);
    }

    #[test]
    #[should_panic]
    fn no_dupes() {
        let mut terrain = RawChunkTerrain::default();
        terrain.add_slab(SlabPointer::new(Slab::empty(0)));
    }

    #[test]
    fn slabs() {
        let mut terrain = RawChunkTerrain::default();

        terrain.add_slab(SlabPointer::new(Slab::empty(1)));
        terrain.add_slab(SlabPointer::new(Slab::empty(2)));

        terrain.add_slab(SlabPointer::new(Slab::empty(-1)));
        terrain.add_slab(SlabPointer::new(Slab::empty(-2)));

        let slabs: Vec<_> = terrain.slabs_from_top().map(|s| s.index()).collect();
        assert_eq!(slabs, vec![2, 1, 0, -1, -2]);

        let slabs: Vec<_> = terrain.slabs_from_bottom().map(|s| s.index()).collect();
        assert_eq!(slabs, vec![-2, -1, 0, 1, 2]);
    }

    #[test]
    fn slab_index() {
        assert_eq!(RawChunkTerrain::slab_index_for_slice(SliceIndex(4)), 0);
        assert_eq!(RawChunkTerrain::slab_index_for_slice(SliceIndex(0)), 0);
        assert_eq!(RawChunkTerrain::slab_index_for_slice(SliceIndex(-3)), -1);
        assert_eq!(RawChunkTerrain::slab_index_for_slice(SliceIndex(-20)), -1);
        assert_eq!(RawChunkTerrain::slab_index_for_slice(SliceIndex(100)), 3);
    }

    #[test]
    fn block_views() {
        let mut terrain = RawChunkTerrain::default();

        *terrain.slice_mut(0).unwrap()[(0, 0)].block_type_mut() = BlockType::Stone;
        assert_eq!(
            terrain.slice(0).unwrap()[(0, 0)].block_type(),
            BlockType::Stone
        );
        assert_eq!(
            terrain.slice(10).unwrap()[(0, 0)].block_type(),
            BlockType::Air
        );

        assert!(terrain.slice(SLAB_SIZE.as_i32()).is_none());
        assert!(terrain.slice(-1).is_none());

        terrain.add_slab(SlabPointer::new(Slab::empty(-1)));
        *terrain.slice_mut(-1).unwrap()[(3, 3)].block_type_mut() = BlockType::Grass;
        assert_eq!(
            terrain.slice(-1).unwrap()[(3, 3)].block_type(),
            BlockType::Grass
        );
        assert_eq!(
            terrain.get_block((3, 3, -1)).unwrap().block_type(),
            BlockType::Grass
        );

        let mut terrain = RawChunkTerrain::default();
        assert_eq!(terrain.try_set_block((2, 0, 0), BlockType::Stone), true);
        assert_eq!(terrain.try_set_block((2, 0, -2), BlockType::Stone), false);
        let mut blocks = Vec::new();
        terrain.blocks(&mut blocks);

        assert_eq!(blocks[0].0, (0, 0, 0).into());
        assert_eq!(blocks[1].0, (1, 0, 0).into());
        assert_eq!(
            blocks
                .iter()
                .filter(|(_, b)| b.block_type() == BlockType::Stone)
                .count(),
            1
        );
    }

    #[test]
    fn slab_areas() {
        // slab with flat slice 0 should have 1 area
        let mut slab = Slab::empty(0);
        slab.slice_mut(0).fill(BlockType::Stone);

        let area_count = AreaDiscovery::from_slab(&slab, None, None).flood_fill_areas();
        assert_eq!(area_count, 1);

        // slab with 2 unconnected floors should have 2
        let mut slab = Slab::empty(0);
        slab.slice_mut(0).fill(BlockType::Stone);
        slab.slice_mut(5).fill(BlockType::Stone);

        let area_count = AreaDiscovery::from_slab(&slab, None, None).flood_fill_areas();
        assert_eq!(area_count, 2);
    }

    #[test]
    fn slab_areas_jump() {
        // terrain with accessible jumps should still be 1 area

        let mut terrain = RawChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll); // solid walkable

        // full jump staircase next to it
        terrain.set_block((3, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((4, 2, 4), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((5, 2, 4), BlockType::Stone, SlabCreationPolicy::CreateAll);

        // 1 area still
        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());
        assert_eq!(terrain.areas.len(), 1);

        // too big jump out of reach is still unreachable
        let mut terrain = RawChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((3, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((4, 2, 7), BlockType::Stone, SlabCreationPolicy::CreateAll);

        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());
        assert_eq!(terrain.areas.len(), 2);

        // if above is blocked, can't jump
        let mut terrain = RawChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((3, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((2, 2, 4), BlockType::Stone, SlabCreationPolicy::CreateAll); // blocks jump!

        // so 2 areas expected
        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());
        assert_eq!(terrain.areas.len(), 2);
    }

    #[test]
    fn cross_slab_walkability() {
        // a slab whose top layer is solid should mean the slab above's z=0 is walkable

        let mut terrain = RawChunkTerrain::default();
        terrain.add_slab(SlabPointer::new(Slab::empty(1))); // add upper slab

        // fill top layer of first slab
        terrain
            .slice_mut(SLAB_SIZE.as_i32() - 1)
            .unwrap()
            .fill(BlockType::Stone);

        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());

        // TODO 1 area at z=0
        assert_eq!(terrain.areas.len(), 1);
    }

    #[test]
    fn create_slab() {
        // setting blocks in non-existent places should create a slab to fill it

        const SLAB_SIZE_I32: i32 = SLAB_SIZE.as_i32();
        let mut terrain = RawChunkTerrain::default();

        // 1 slab below should not yet exist
        assert!(!terrain.set_block((0, 0, -5), BlockType::Stone, SlabCreationPolicy::PleaseDont));
        assert!(terrain.get_block((0, 0, -5)).is_none());
        assert_eq!(terrain.slab_count(), 1);
        assert_eq!(
            terrain.slice_bounds_as_slabs(),
            SliceRange::from_bounds(0, SLAB_SIZE_I32)
        );

        // now really set
        assert!(terrain.set_block((0, 0, -5), BlockType::Stone, SlabCreationPolicy::CreateAll));
        assert_eq!(
            terrain
                .get_block((0, 0, -5))
                .map(|b| b.block_type())
                .unwrap(),
            BlockType::Stone
        );
        assert_eq!(terrain.slab_count(), 2);
        assert_eq!(
            terrain.slice_bounds_as_slabs(),
            SliceRange::from_bounds(-SLAB_SIZE_I32, SLAB_SIZE_I32)
        );

        // set a high block that will fill the rest in with air
        assert!(terrain.set_block((0, 0, 100), BlockType::Grass, SlabCreationPolicy::CreateAll));
        assert_eq!(
            terrain
                .get_block((0, 0, 100))
                .map(|b| b.block_type())
                .unwrap(),
            BlockType::Grass
        );
        assert_eq!(terrain.slab_count(), 5);
        assert!(terrain.slice_bounds_as_slabs().contains(100));

        for z in 0..100 {
            // air inbetween
            assert_eq!(
                terrain
                    .get_block((0, 0, z))
                    .map(|b| b.block_type())
                    .unwrap(),
                BlockType::Air
            );
        }
    }

    #[test]
    fn block_graph_high_jump() {
        // there should be no edge that is a jump of > 1.0

        let terrain = ChunkBuilder::new()
            .set_block((2, 2, 2), BlockType::Stone)
            // technically a vertical neighbour but the jump is too high
            .set_block((3, 2, 4), BlockType::Stone)
            .into_inner();

        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());
        assert_eq!(terrain.areas.len(), 2); // 2 disconnected areas
    }

    #[test]
    fn discovery_block_graph() {
        let terrain = ChunkBuilder::new()
            .fill_slice(1, BlockType::Stone)
            .set_block((2, 2, 2), BlockType::Grass)
            .into_inner();

        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());

        let graph = terrain
            .block_graph_for_area(WorldArea::new((0, 0)))
            .unwrap();

        // 4 flat connections
        assert_eq!(
            graph.edges((5, 5, 2).into()),
            vec![
                ((4, 5, 2).into(), EdgeCost::Walk),
                ((5, 4, 2).into(), EdgeCost::Walk),
                ((5, 6, 2).into(), EdgeCost::Walk),
                ((6, 5, 2).into(), EdgeCost::Walk),
            ]
        );

        // step up on 1 side
        assert_eq!(
            graph.edges((2, 3, 2).into()),
            vec![
                ((1, 3, 2).into(), EdgeCost::Walk),
                ((2, 2, 3).into(), EdgeCost::JumpUp),
                ((2, 4, 2).into(), EdgeCost::Walk),
                ((3, 3, 2).into(), EdgeCost::Walk),
            ]
        );

        // step down on all sides
        assert_eq!(
            graph.edges((2, 2, 3).into()),
            vec![
                ((1, 2, 2).into(), EdgeCost::JumpDown),
                ((2, 1, 2).into(), EdgeCost::JumpDown),
                ((2, 3, 2).into(), EdgeCost::JumpDown),
                ((3, 2, 2).into(), EdgeCost::JumpDown),
            ]
        );
    }

    #[test]
    fn slice_index_in_slab() {
        // positives are simple modulus
        assert_eq!(
            RawChunkTerrain::slice_index_in_slab(SliceIndex(5)),
            SliceIndex(5)
        );
        assert_eq!(
            RawChunkTerrain::slice_index_in_slab(SliceIndex(SLAB_SIZE.as_i32() + 4)),
            SliceIndex(4)
        );

        // negatives work backwards
        assert_eq!(
            RawChunkTerrain::slice_index_in_slab(SliceIndex(-1)),
            SliceIndex(SLAB_SIZE.as_i32() - 1)
        );
    }

    #[test]
    fn slice_index_in_chunk() {
        assert_eq!(
            RawChunkTerrain::slice_index_in_chunk(0, SliceIndex(5)),
            SliceIndex(5)
        );
        assert_eq!(
            RawChunkTerrain::slice_index_in_chunk(1, SliceIndex(5)),
            SliceIndex(SLAB_SIZE.as_i32() + 5)
        );

        assert_eq!(
            RawChunkTerrain::slice_index_in_chunk(-1, SliceIndex(0)),
            SliceIndex(-SLAB_SIZE.as_i32())
        );
        assert_eq!(
            RawChunkTerrain::slice_index_in_chunk(-1, SliceIndex(1)),
            SliceIndex(-SLAB_SIZE.as_i32() + 1)
        );
    }

    #[test]
    fn occlusion_in_slab() {
        // no occlusion because the block directly above 2,2,2 is solid
        let mut terrain = RawChunkTerrain::default();
        assert!(terrain.set_block((2, 2, 2), BlockType::Dirt, SlabCreationPolicy::CreateAll));
        assert!(terrain.set_block((2, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll));
        assert!(terrain.set_block((2, 3, 3), BlockType::Dirt, SlabCreationPolicy::CreateAll));
        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());

        let occlusion = *terrain.get_block((2, 2, 2)).unwrap().occlusion();
        assert_matches!(occlusion.corner(0), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(1), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(2), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(3), VertexOcclusion::NotAtAll);

        // occlusion will be populated if block directly above it is air
        let mut terrain: RawChunkTerrain = terrain.into();
        assert!(terrain.set_block((2, 2, 3), BlockType::Air, SlabCreationPolicy::CreateAll));
        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());

        let occlusion = *terrain.get_block((2, 2, 2)).unwrap().occlusion();
        assert_matches!(occlusion.corner(0), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(1), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(2), VertexOcclusion::Mildly);
        assert_matches!(occlusion.corner(3), VertexOcclusion::Mildly);
    }

    #[test]
    fn occlusion_across_slab() {
        let mut terrain = RawChunkTerrain::default();
        assert!(terrain.set_block(
            (2, 2, SLAB_SIZE.as_i32() - 1),
            BlockType::Dirt,
            SlabCreationPolicy::CreateAll,
        ));
        assert!(terrain.set_block(
            (2, 3, SLAB_SIZE.as_i32()),
            BlockType::Dirt,
            SlabCreationPolicy::CreateAll,
        )); // next slab up

        let terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());

        let occlusion = *terrain
            .get_block((2, 2, SLAB_SIZE.as_i32() - 1))
            .unwrap()
            .occlusion();
        assert_matches!(occlusion.corner(0), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(1), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(2), VertexOcclusion::Mildly);
        assert_matches!(occlusion.corner(3), VertexOcclusion::Mildly);
    }

    fn occlusion(
        world: &World,
        chunk: ChunkPosition,
        block: (i32, i32, i32),
    ) -> [VertexOcclusion; 4] {
        let chunk = world.find_chunk_with_pos(chunk).unwrap();
        let block = chunk.get_block(block).unwrap();
        let occlusion = block.occlusion();

        [
            occlusion.corner(0),
            occlusion.corner(1),
            occlusion.corner(2),
            occlusion.corner(3),
        ]
    }

    #[test]
    fn occlusion_across_chunk_sides() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let a = ChunkBuilder::new()
            .set_block((0, 0, SLAB_SIZE.as_i32()), BlockType::Grass) // slab 1
            .set_block((0, 0, 0), BlockType::Grass) // slab 0
            .build((0, 0));

        let b = ChunkBuilder::new()
            // occludes 0,0,0 in slab 0
            .set_block((CHUNK_SIZE.as_i32() - 1, 0, 1), BlockType::Stone)
            .build((-1, 0));

        // occludes 0,0,0 in slab 0
        let c = ChunkBuilder::new()
            .set_block((0, CHUNK_SIZE.as_i32() - 1, 1), BlockType::Stone)
            .build((0, -1));

        let world = world_from_chunks(vec![a, b, c]).into_inner();

        // 0,0,0 occluded by 2 chunk neighbours
        assert!(world.block((-1, 0, 1)).unwrap().opacity().solid());
        assert!(world.block((0, -1, 1)).unwrap().opacity().solid());
        assert_matches!(
            occlusion(&world, ChunkPosition(0, 0), (0, 0, 0)),
            [
                VertexOcclusion::Mostly,
                VertexOcclusion::Mildly,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly
            ]
        );
    }

    #[test]
    fn lazy_occlusion_top_only() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        fn mk_chunks(block_off: bool) -> World {
            let a = ChunkBuilder::new()
                .set_block((CHUNK_SIZE.as_i32() - 1, 0, 0), BlockType::Grass)
                .build((-1, 0));

            let blockage = if block_off {
                BlockType::Grass
            } else {
                BlockType::Air
            };

            let b = ChunkBuilder::new()
                .set_block((0, 0, -1), BlockType::Stone)
                .set_block((1, 0, 0), BlockType::Stone)
                .set_block((0, 0, 0), blockage)
                .build((0, 0));

            world_from_chunks(vec![a, b]).into_inner()
        }

        let world = mk_chunks(false);

        // 0, 0, -1 occluded by (-1, 0, 0) in other chunk and (1, 0, 0) in other slab
        assert!(world.block((0, 0, -1)).unwrap().opacity().solid());
        assert!(world.block((-1, 0, 0)).unwrap().opacity().solid());
        assert!(world.block((0, 0, 0)).unwrap().opacity().transparent());
        assert_matches!(
            occlusion(&world, ChunkPosition(0, 0), (0, 0, -1)),
            [
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly
            ]
        );

        // ... but when (0,0,0) is solid, (0,0,-1) is hidden so it shouldnt be updated
        let world = mk_chunks(true);

        assert!(world.block((0, 0, 0)).unwrap().opacity().solid());
        assert_matches!(
            occlusion(&world, ChunkPosition(0, 0), (0, 0, -1)),
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
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let a = ChunkBuilder::new()
            // 0, 15, 0
            .set_block((0, CHUNK_SIZE.as_i32() - 1, 0), BlockType::Stone)
            .build((0, 0));

        let b = ChunkBuilder::new()
            // -1, 16, 1, occludes 0,0,0
            .set_block((CHUNK_SIZE.as_i32() - 1, 0, 1), BlockType::Grass)
            .build((-1, 1));

        let c = ChunkBuilder::new()
            // 0, 16, 1, occludes 0,0,0
            .set_block((0, 0, 1), BlockType::Grass)
            .build((0, 1));

        let world = world_from_chunks(vec![a, b, c]).into_inner();

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
        assert_matches!(
            occlusion(&world, ChunkPosition(0, 0), (0, CHUNK_SIZE.as_i32() - 1, 0)),
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mostly,
            ]
        );
    }

    #[test]
    fn ascending_slice_pairs() {
        let mut terrain = RawChunkTerrain::default();

        // this pattern should repeat across slices across slabs
        const PATTERN: [BlockType; 4] = [
            BlockType::Air,
            BlockType::Dirt,
            BlockType::Stone,
            BlockType::Grass,
        ];

        // crosses 3 slabs
        for (bt, z) in PATTERN
            .iter()
            .cycle()
            .zip(-SLAB_SIZE.as_i32()..(SLAB_SIZE.as_i32() * 2) - 1)
        {
            terrain.set_block((0, 0, z), *bt, SlabCreationPolicy::CreateAll);
        }

        let mut count = 0;
        let mut expected = PATTERN.iter().cycle().copied().peekable();

        let mut terrain = ChunkTerrain::from_raw_terrain(terrain, (0, 0).into());
        assert_eq!(terrain.raw_terrain().slab_count(), 3 + 1); // 3 + 1 empty at the top

        const EXPECTED_COUNT: i32 = (SLAB_SIZE.as_i32() * 4) - 1;
        const PATTERN_SLICE_LIMIT: i32 = (SLAB_SIZE.as_i32() * 3) - 1;

        terrain.ascending_slice_pairs_foreach(|a, b| {
            count += 1;

            let expected_next = if count > PATTERN_SLICE_LIMIT {
                BlockType::Air // empty slab at the top is all air
            } else {
                expected.next().unwrap()
            };

            assert_eq!(dbg!(a[(0, 0)].block_type()), expected_next);

            let expected_next = if count >= PATTERN_SLICE_LIMIT {
                BlockType::Air // top of highest slab with nothing above it
            } else {
                *expected.peek().unwrap()
            };

            assert_eq!(dbg!(b[(0, 0)].block_type()), expected_next);
        });

        assert_eq!(count, EXPECTED_COUNT);
    }
}
