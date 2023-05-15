use std::cmp::Ordering;
use std::f32::EPSILON;
use std::fmt::{Debug, Formatter};
use std::hint::unreachable_unchecked;
use std::iter::{once, repeat};
use std::num::NonZeroU16;
use std::sync::Arc;
use std::time::Instant;

use arbitrary_int::{u4, u5};
use md5::digest::FixedOutput;
use misc::FmtResult;
use misc::*;
use unit::world::{
    BlockCoord, BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex,
    SlabPosition, SliceIndex, SLAB_SIZE,
};
use unit::world::{SliceBlock, CHUNK_SIZE};

use crate::block::BlockDurability;
use crate::block::{Block, BlockEnriched};
use crate::chunk::double_sided_vec::DoubleSidedVec;
use crate::chunk::slab::Slab;
use crate::chunk::slab::{DeepClone, SliceNavArea};
use crate::chunk::slice::{Slice, SliceMut};
use crate::chunk::{AreaInfo, Chunk};
use crate::helpers::DummyWorldContext;
use crate::loader::SlabVerticalSpace;
use crate::navigation::{ChunkArea, SlabAreaIndex};
use crate::navigationv2::{is_border, is_bottom_area, is_top_area, SlabArea, SlabNavGraph};
use crate::neighbour::NeighbourOffset;
use crate::occlusion::{BlockOcclusionUpdate, NeighbourOpacity};
use crate::{BlockOcclusion, BlockType, EdgeCost, OcclusionFace, SliceRange, WorldContext};

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub enum SlabNeighbour {
    Top = 0,
    Bottom,
    North,
    East,
    South,
    West,
}

impl SlabNeighbour {
    const N: usize = 6;
    pub const VALUES: [SlabNeighbour; SlabNeighbour::N] = [
        Self::Top,
        Self::Bottom,
        Self::North,
        Self::East,
        Self::South,
        Self::West,
    ];

    pub fn is_border(&self, area: &SliceNavArea) -> bool {
        use SlabNeighbour::*;
        let dir = match self {
            Top => return is_top_area(area),
            Bottom => return is_bottom_area(area),
            North => NeighbourOffset::North,
            East => NeighbourOffset::East,
            South => NeighbourOffset::South,
            West => NeighbourOffset::West,
        };

        is_border(dir, (area.from, area.to))
    }
}

#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub struct NeighbourAreaHash([u8; 16]);

impl NeighbourAreaHash {
    pub fn for_areas_with_edge(
        edge: SlabNeighbour,
        areas: impl Iterator<Item = SliceNavArea>,
    ) -> Self {
        Self::for_areas(areas.filter(|a| edge.is_border(a)))
    }

    pub fn for_areas(areas: impl Iterator<Item = SliceNavArea>) -> Self {
        use md5::Digest;
        let mut h = md5::Md5::new();
        for a in areas {
            let arr: [u8; 6] = [
                a.slice.slice(),
                a.height,
                a.from.0,
                a.from.1,
                a.to.0,
                a.to.1,
            ];
            h.update(&arr);
        }
        let res = h.finalize_fixed();
        Self(res.into())
    }
}

impl Debug for NeighbourAreaHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "NeighbourAreaHash(")?;
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        write!(f, ")")
    }
}

/// Maps T to a block in a slab
#[derive(Derivative, Clone)]
#[derivative(Default(bound = ""))]
pub struct SparseGrid<T>(Vec<(PackedSlabPosition, T)>);

pub struct SparseGridExtension<'a, T> {
    grid: &'a mut SparseGrid<T>,
    needs_sort: bool,
}

impl<T> SparseGrid<T> {
    pub fn new_unsorted(mut positions: Vec<(PackedSlabPosition, T)>) -> Self {
        positions.sort_unstable_by_key(|(k, _)| *k);
        Self(positions)
    }

    pub fn iter(&self) -> impl Iterator<Item = (SlabPosition, &T)> + '_ {
        self.0.iter().map(|(pos, t)| ((*pos).into(), t))
    }

    fn find(&self, pos: SlabPosition) -> Option<usize> {
        self.0
            .binary_search_by_key(&PackedSlabPosition::from(pos), |(k, _)| *k)
            .ok()
    }

    pub fn contains(&self, pos: SlabPosition) -> bool {
        self.find(pos).is_some()
    }

    pub fn get(&self, pos: SlabPosition) -> Option<&T> {
        self.find(pos)
            .map(|i| unsafe { &self.0.get_unchecked(i).1 })
    }

    pub fn get_mut(&mut self, pos: SlabPosition) -> Option<&mut T> {
        self.find(pos)
            .map(|i| unsafe { &mut self.0.get_unchecked_mut(i).1 })
    }

    pub fn extend(&mut self) -> SparseGridExtension<T> {
        SparseGridExtension {
            grid: self,
            needs_sort: false,
        }
    }
}

impl<T: Default> SparseGridExtension<'_, T> {
    /// Must not exist already
    pub fn add_new(&mut self, pos: SlabPosition, val: T) {
        debug_assert!(!self.grid.contains(pos));

        self.grid.0.push((pos.into(), val));
        self.needs_sort = true;
    }

    pub fn get_or_insert(&mut self, pos: SlabPosition) -> &mut T {
        let idx = match self.grid.find(pos) {
            None => {
                self.add_new(pos, T::default());
                self.grid.0.len() - 1
            }
            Some(i) => i,
        };
        unsafe { &mut self.grid.0.get_unchecked_mut(idx).1 }
    }

    pub fn remove(&mut self, pos: SlabPosition) {
        if let Some(idx) = self.grid.find(pos) {
            self.grid.0.swap_remove(idx);
            self.needs_sort = true;
        }
    }
}

impl<T> Drop for SparseGridExtension<'_, T> {
    fn drop(&mut self) {
        if self.needs_sort {
            self.grid.0.sort_unstable_by_key(|(k, _)| *k);
        }
    }
}

#[bitbybit::bitfield(u16, default: 0)]
#[derive(Eq, PartialEq)]
pub struct PackedSlabPosition {
    #[bits(12..=15, rw)]
    x: u4,

    #[bits(8..=11, rw)]
    y: u4,

    #[bits(3..=7, rw)]
    z: u5,
}

impl From<PackedSlabPosition> for SlabPosition {
    fn from(packed: PackedSlabPosition) -> Self {
        SlabPosition::new_unchecked(
            packed.x().value(),
            packed.y().value(),
            LocalSliceIndex::new_srsly_unchecked(packed.z().value()),
        )
    }
}

impl From<SlabPosition> for PackedSlabPosition {
    fn from(pos: SlabPosition) -> Self {
        unsafe {
            PackedSlabPosition::new()
                .with_x(u4::new_unchecked(pos.x()))
                .with_y(u4::new_unchecked(pos.y()))
                .with_z(u5::new_unchecked(pos.z().slice()))
        }
    }
}

impl PartialOrd for PackedSlabPosition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackedSlabPosition {
    fn cmp(&self, other: &Self) -> Ordering {
        self.z()
            .cmp(&other.z())
            .then(self.y().cmp(&other.y()))
            .then(self.x().cmp(&other.x()))
    }
}

impl Debug for PackedSlabPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("PackedSlabPosition")
            .field(&SlabPosition::from(*self))
            .finish()
    }
}

pub struct SlabData<C: WorldContext> {
    pub(crate) terrain: Slab<C>,
    pub(crate) nav: SlabNavGraph, // currently unused
    pub(crate) vertical_space: Arc<SlabVerticalSpace>,
    pub(in crate::chunk) last_modify_time: Instant,
    /// Current version of this slab
    pub(in crate::chunk) version: SlabVersion,
    /// Version of each neighbour (indexed by SlabNeighbour) at load time, None if it was not loaded
    pub(in crate::chunk) neighbour_versions: [Option<SlabVersion>; SlabNeighbour::N],

    pub(crate) neighbour_edge_hashes: [NeighbourAreaHash; SlabNeighbour::N],

    pub(crate) occlusion: SparseGrid<BlockOcclusion>,
}

/// Wrapping version for a slab to detect changes. Incremented each time it is modified
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct SlabVersion(NonZeroU16);

#[derive(Copy, Clone)]
pub enum SlabCreationPolicy {
    /// Don't add missing slabs
    PleaseDont,

    /// Create the missing slab and all intermediate slabs
    CreateAll { placeholders: bool },
}

pub enum BlockDamageResult {
    Broken,
    Unbroken,
}

impl<C: WorldContext> SlabStorage<C> {
    pub fn slice<S: Into<GlobalSliceIndex>>(&self, index: S) -> Option<Slice<C>> {
        let chunk_slice_idx = index.into();
        let slab_idx = chunk_slice_idx.slab_index();
        self.slabs
            .get(slab_idx)
            .map(|data| data.terrain.slice(chunk_slice_idx.to_local()))
    }

    pub fn slice_unchecked<S: Into<GlobalSliceIndex>>(&self, index: S) -> Slice<C> {
        // TODO actually add get_{mut_}unchecked to slabs for performance
        self.slice(index).unwrap()
    }

    /// Panics if not exclusive reference
    pub fn slice_mut<S: Into<GlobalSliceIndex>>(&mut self, index: S) -> Option<SliceMut<C>> {
        let chunk_slice_idx = index.into();
        let slab_idx = chunk_slice_idx.slab_index();
        self.slabs
            .get_mut(slab_idx)
            .map(|data| data.terrain.slice_mut(chunk_slice_idx.to_local()))
    }

    pub fn get_block(&self, pos: BlockPosition) -> Option<Block<C>> {
        self.slice(pos.z()).map(|slice| slice[pos])
    }

    /// Panics if invalid position
    #[cfg(test)]
    pub fn get_block_tup(&self, pos: (i32, i32, i32)) -> Option<BlockEnriched<C>> {
        let pos = BlockPosition::try_from(pos).expect("bad position");
        let block = self.slice(pos.z()).map(|slice| slice[pos])?;

        let data = self.slab_data(pos.z().slab_index()).expect("bad slab");
        let occlusion = data
            .occlusion
            .get(SlabPosition::from(pos))
            .copied()
            .unwrap_or_default();
        Some(BlockEnriched {
            block_type: block.block_type(),
            occlusion,
        })
    }

    /// Only for tests
    #[cfg(test)]
    pub fn blocks<'a>(
        &self,
        out: &'a mut Vec<(BlockPosition, Block<C>)>,
    ) -> &'a mut Vec<(BlockPosition, Block<C>)> {
        let (_bottom_slab, bottom_slab_index) = self.slabs_from_bottom().next().unwrap();

        let SlabIndex(low_z) = bottom_slab_index * SLAB_SIZE;
        let high_z = low_z + (self.slab_count() * SLAB_SIZE.as_usize()) as i32;

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

    pub(crate) fn slabs_from_top(&self) -> impl Iterator<Item = (&Slab<C>, SlabIndex)> {
        self.slabs
            .iter_decreasing()
            .zip(self.slabs.indices_decreasing())
            .map(|(data, idx)| (&data.terrain, SlabIndex(idx)))
    }

    pub(crate) fn slabs_from_bottom(&self) -> impl Iterator<Item = (&Slab<C>, SlabIndex)> {
        self.slabs
            .iter_increasing()
            .zip(self.slabs.indices_increasing())
            .map(|(data, idx)| (&data.terrain, SlabIndex(idx)))
    }

    pub(crate) fn slab_data_from_top(&self) -> impl Iterator<Item = (&SlabData<C>, SlabIndex)> {
        self.slabs
            .iter_decreasing()
            .zip(self.slabs.indices_decreasing())
            .map(|(data, idx)| (data, SlabIndex(idx)))
    }

    pub(crate) fn slab_data_from_bottom(&self) -> impl Iterator<Item = (&SlabData<C>, SlabIndex)> {
        self.slabs
            .iter_increasing()
            .zip(self.slabs.indices_increasing())
            .map(|(data, idx)| (data, SlabIndex(idx)))
    }

    /// Adds slab, returning old if it exists
    pub fn replace_slab(&mut self, index: SlabIndex, new_slab: SlabData<C>) -> Option<SlabData<C>> {
        if let Some(old) = self.slabs.get_mut(index) {
            Some(std::mem::replace(old, new_slab))
        } else {
            self.slabs.add(new_slab, index);
            None
        }
    }

    #[cfg(test)]
    pub fn add_empty_placeholder_slab(&mut self, slab: impl Into<SlabIndex>) {
        self.slabs.add(SlabData::new(Slab::empty()), slab.into());
    }

    pub fn slab_data(&self, index: SlabIndex) -> Option<&SlabData<C>> {
        self.slabs.get(index)
    }

    pub fn slab_data_mut(&mut self, index: SlabIndex) -> Option<&mut SlabData<C>> {
        self.slabs.get_mut(index)
    }

    pub fn slab(&self, index: SlabIndex) -> Option<&Slab<C>> {
        self.slabs.get(index).map(|s| &s.terrain)
    }

    /// Cow-copies the slab if not already the exclusive holder
    pub(crate) fn slab_mut(&mut self, index: SlabIndex) -> Option<&mut Slab<C>> {
        self.slabs.get_mut(index).map(|s| s.terrain.cow_clone())
    }

    pub(crate) fn copy_slab(&self, index: SlabIndex) -> Option<Slab<C>> {
        self.slabs.get(index).map(|s| s.terrain.deep_clone())
    }

    /// Fills in gaps in slabs up to the inclusive target with empty placeholder slabs. Nop if zero
    pub(crate) fn create_slabs_until(&mut self, target: SlabIndex) {
        if target != SlabIndex(0) {
            self.slabs
                .fill_until(target, |_| SlabData::new(Slab::empty()))
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
            slab.slices_from_top()
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
                let lower_hi = lower.terrain.slice(LocalSliceIndex::top());
                let upper_lo = upper.terrain.slice(LocalSliceIndex::bottom());

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
                SlabCreationPolicy::CreateAll { placeholders } if try_again => {
                    // create slabs
                    self.create_slabs_until(slice.slab_index());

                    if !placeholders {
                        // make all slabs unique, could be more efficient but not used in actual game
                        let (from, to) = self.slab_range();
                        for idx in from.0..=to.0 {
                            let _ = self.slab_mut(SlabIndex(idx));
                        }
                    }

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

    pub fn deep_clone(&self) -> Self {
        Self {
            slabs: self.slabs.deep_clone(),
        }
    }

    /// Returns iter of slabs affected, probably duplicated but in order
    #[deprecated]
    pub fn apply_occlusion_updates<'a>(
        &'a mut self,
        updates: &'a [(BlockPosition, BlockOcclusionUpdate)],
    ) -> impl Iterator<Item = SlabIndex> + 'a {
        updates
            .iter()
            .filter_map(move |&(block_pos, new_opacities)| unreachable!())
    }

    /// Searches downwards
    #[deprecated]
    pub fn find_accessible_block(
        &self,
        pos: SliceBlock,
        start_from: Option<GlobalSliceIndex>,
        end_at: Option<GlobalSliceIndex>,
    ) -> Option<BlockPosition> {
        unreachable!("unimpl");
        self.find_from_top(pos, start_from, end_at, |above, below| {
            above.walkable() && below.block_type().can_be_walked_on()
        })
    }

    /// Just finds a block with air above it
    pub fn find_guessed_ground_level(
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

impl SlabVersion {
    const DEFAULT: NonZeroU16 = unsafe { NonZeroU16::new_unchecked(1) };
    // pub fn get(self) -> NonZeroU16{self.0}

    pub fn inc(&mut self) {
        self.0 = self.0.checked_add(1).unwrap_or(Self::DEFAULT);
    }
}

impl Default for SlabVersion {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl<C: WorldContext> SlabData<C> {
    pub(crate) fn new(terrain: Slab<C>) -> Self {
        Self {
            terrain,
            nav: SlabNavGraph::empty(),
            vertical_space: SlabVerticalSpace::empty(),
            last_modify_time: Instant::now(),
            version: Default::default(),
            neighbour_versions: [None; SlabNeighbour::N],
            neighbour_edge_hashes: [NeighbourAreaHash::default(); SlabNeighbour::N],
            occlusion: SparseGrid::default(),
        }
    }
}

impl<C: WorldContext> DeepClone for SlabData<C> {
    fn deep_clone(&self) -> Self {
        Self {
            terrain: self.terrain.deep_clone(),
            nav: self.nav.clone(),
            vertical_space: self.vertical_space.clone(),
            last_modify_time: self.last_modify_time,
            version: self.version,
            neighbour_versions: self.neighbour_versions,
            neighbour_edge_hashes: self.neighbour_edge_hashes,
            occlusion: self.occlusion.clone(),
        }
    }
}

impl<C: WorldContext> SlabData<C> {
    /// Bumps slab version and sets last modified time to now. Returns new version
    pub fn mark_modified(&mut self) -> SlabVersion {
        self.last_modify_time = Instant::now();
        self.version.inc();
        self.version
    }
}

/// Clone with `deep_clone`
pub struct SlabStorage<C: WorldContext> {
    slabs: DoubleSidedVec<SlabData<C>>,
}

impl<C: WorldContext> Default for SlabStorage<C> {
    /// Has a single empty slab at index 0
    fn default() -> Self {
        let mut terrain = Self {
            slabs: DoubleSidedVec::with_capacity(8),
        };

        terrain.slabs.add(SlabData::new(Slab::empty()), 0);

        terrain
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use unit::world::{GlobalSliceIndex, WorldPositionRange, SLAB_SIZE};
    use unit::world::{WorldPosition, CHUNK_SIZE};

    use crate::chunk::slab::Slab;
    use crate::chunk::ChunkBuilder;
    use crate::helpers::{loader_from_chunks_blocking, DummyBlockType};
    use crate::loader::WorldTerrainUpdate;
    use crate::occlusion::{OcclusionFace, VertexOcclusion};
    use crate::world::helpers::{
        apply_updates, load_single_chunk, world_from_chunks_blocking, DummyWorldContext,
    };
    use crate::{World, WorldArea, WorldRef};

    use super::*;

    #[test]
    fn empty() {
        let terrain = SlabStorage::<DummyWorldContext>::default();
        assert_eq!(terrain.slab_count(), 1);
    }

    #[test]
    #[should_panic]
    fn no_dupes() {
        let mut terrain = SlabStorage::<DummyWorldContext>::default();
        terrain.add_empty_placeholder_slab(0);
    }

    #[test]
    fn slabs() {
        let mut terrain = SlabStorage::<DummyWorldContext>::default();

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
        let mut terrain = SlabStorage::<DummyWorldContext>::default();

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

        let mut terrain = SlabStorage::<DummyWorldContext>::default();
        assert_eq!(
            terrain.set_block(
                (2, 0, 0).try_into().unwrap(),
                DummyBlockType::Stone,
                SlabCreationPolicy::PleaseDont,
            ),
            true
        );
        assert_eq!(
            terrain.set_block(
                (2, 0, -2).try_into().unwrap(),
                DummyBlockType::Stone,
                SlabCreationPolicy::PleaseDont,
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
        let mut terrain = SlabStorage::<DummyWorldContext>::default();

        // 1 slab below should not yet exist
        assert!(!terrain.set_block(
            (0, 0, -5).try_into().unwrap(),
            DummyBlockType::Stone,
            SlabCreationPolicy::PleaseDont,
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
            SlabCreationPolicy::CreateAll { placeholders: true },
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
            SlabCreationPolicy::CreateAll { placeholders: true },
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
            .terrain()
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
            .terrain()
            .get_block_tup((2, 2, 2))
            .unwrap()
            .occlusion()
            .resolve_vertices(OcclusionFace::Top);

        assert_eq!(
            occlusion,
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly,
                VertexOcclusion::NotAtAll,
            ]
        );
    }

    #[test]
    fn occlusion_across_slab() {
        let terrain = ChunkBuilder::default()
            .set_block((2, 2, SLAB_SIZE.as_i32() - 1), DummyBlockType::Dirt)
            .set_block(
                (2, 3, SLAB_SIZE.as_i32()), // next slab up
                DummyBlockType::Dirt,
            );

        let terrain = load_single_chunk(terrain);

        let below_block_occlusion = *terrain
            .terrain()
            .get_block_tup((2, 2, SLAB_SIZE.as_i32() - 1))
            .unwrap()
            .occlusion();

        let above_block_occlusion = *terrain
            .terrain()
            .get_block_tup((2, 2, SLAB_SIZE.as_i32()))
            .unwrap()
            .occlusion();

        // below should have top like this and the rest not
        assert_eq!(
            below_block_occlusion.resolve_vertices(OcclusionFace::Top).0,
            [
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
                VertexOcclusion::Mildly,
                VertexOcclusion::NotAtAll,
            ]
        );

        for f in OcclusionFace::FACES
            .iter()
            .filter(|f| !matches!(**f, OcclusionFace::Top))
        {
            assert!(
                below_block_occlusion.get_face(*f).is_all_transparent(),
                "not all transparent for face {:?} - {:?}",
                f,
                below_block_occlusion.get_face(*f)
            );
        }

        // above should have south like this and the rest not
        assert_eq!(
            above_block_occlusion
                .resolve_vertices(OcclusionFace::South)
                .0,
            [
                VertexOcclusion::Mildly,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
            ]
        );

        for f in OcclusionFace::FACES
            .iter()
            .filter(|f| !matches!(**f, OcclusionFace::South))
        {
            assert!(
                above_block_occlusion.get_face(*f).is_all_transparent(),
                "not all transparent for face {:?} - {:?}",
                f,
                above_block_occlusion.get_face(*f)
            );
        }

        // above should have south like this and the rest not
    }

    fn occlusion(
        world: &World<DummyWorldContext>,
        chunk: ChunkLocation,
        block: (i32, i32, i32),
        face: OcclusionFace,
    ) -> [VertexOcclusion; 4] {
        let chunk = world.find_chunk_with_pos(chunk).unwrap();
        let occlusion = chunk
            .terrain()
            .slab_data(GlobalSliceIndex::new(block.2).slab_index())
            .expect("bad slab")
            .occlusion
            .get(WorldPosition::from(block).into())
            .expect("no occlusion for block");
        occlusion.resolve_vertices(face).0
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
                VertexOcclusion::Mildly,
                VertexOcclusion::NotAtAll,
                VertexOcclusion::Mildly,
                VertexOcclusion::Full,
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
                VertexOcclusion::Mildly,
                VertexOcclusion::Mostly,
                VertexOcclusion::NotAtAll,
            ]
        ));
    }

    #[test]
    fn cloned_slab_cow_is_updated() {
        let mut terrain = SlabStorage::<DummyWorldContext>::default();
        let old = terrain.slabs.get(0).unwrap().terrain.clone(); // reference to force a cow clone

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

        // remove some of the wall
        let updates = [WorldTerrainUpdate::new(
            WorldPositionRange::with_inclusive_range((3, 0, 1), (3, 0, 2)),
            DummyBlockType::Air,
        )];
        apply_updates(&mut loader, &updates).expect("updates failed");

        assert_eq!(get_areas().len(), 4);

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

    #[test]
    fn lookup_pos_to_area() {
        let mut loader =
            loader_from_chunks_blocking(vec![ChunkBuilder::<DummyWorldContext>::new()
                .set_block((2, 2, 0), DummyBlockType::Dirt)
                .set_block((2, 2, 4), DummyBlockType::Dirt)
                .set_block((2, 2, 5), DummyBlockType::Dirt)
                .build((0, 0))]);

        let world_ref = loader.world();
        let w = world_ref.borrow();
        let chunk = w.find_chunk_with_pos((0, 0).into()).unwrap();

        let get_block = |slice| {
            chunk
                .find_area_for_block_with_height(
                    BlockPosition::new(2, 2, GlobalSliceIndex::new(slice)).unwrap(),
                    3,
                )
                .map(|tup| tup.0)
        };
        assert_eq!(get_block(0), None); // solid
        let the_area = get_block(1).expect("should have area");
        assert_eq!(get_block(2), Some(the_area));
        assert_eq!(get_block(3), Some(the_area));
        assert_eq!(get_block(4), None); // solid
        assert_eq!(get_block(5), None); // solid

        let above_area = get_block(6);
        assert!(above_area.is_some());
        assert_ne!(above_area, Some(the_area));

        assert_eq!(
            chunk.find_area_for_block_with_height(
                BlockPosition::new(2, 2, GlobalSliceIndex::new(3)).unwrap(),
                4
            ),
            None
        ); // too tall
    }

    #[test]
    fn sparse_grid() {
        let blocks = [
            SlabPosition::new_unchecked(5, 5, LocalSliceIndex::new_srsly_unchecked(2)),
            SlabPosition::new_unchecked(1, 5, LocalSliceIndex::new_srsly_unchecked(6)),
            SlabPosition::new_unchecked(5, 2, LocalSliceIndex::new_srsly_unchecked(1)),
            SlabPosition::new_unchecked(3, 3, LocalSliceIndex::new_srsly_unchecked(0)),
        ];

        let vals = [1, 2, 3, 4];

        let mut grid = SparseGrid::new_unsorted(
            blocks
                .into_iter()
                .map(|p| PackedSlabPosition::from(p))
                .zip(vals)
                .collect_vec(),
        );

        for (b, v) in blocks.iter().zip(vals) {
            assert_eq!(grid.get(*b).copied(), Some(v));
        }

        assert!(!grid.contains(SlabPosition::new_unchecked(
            3,
            3,
            LocalSliceIndex::new_srsly_unchecked(2)
        )));

        let new_block = SlabPosition::new_unchecked(8, 3, LocalSliceIndex::new_srsly_unchecked(0));
        let mut ext = grid.extend();
        ext.remove(blocks[0]);
        *ext.get_or_insert(new_block) = 10;
        drop(ext);

        assert_eq!(grid.iter().count(), blocks.len());
        assert!(!grid.contains(blocks[0]));
        assert_eq!(grid.get(new_block).copied(), Some(10));
    }
}
