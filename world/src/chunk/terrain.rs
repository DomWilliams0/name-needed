use std::collections::HashMap;
use std::ops::Deref;

use generator::{done, Generator, Gn};
use itertools::Itertools;
use log::debug;

use crate::area::discovery::AreaDiscovery;
use crate::area::{Area, ChunkBoundary, SlabArea};
use crate::block::Block;
use crate::chunk::double_sided_vec::DoubleSidedVec;
use crate::chunk::slab::{Slab, SlabIndex, SLAB_SIZE};
use crate::chunk::slice::{Slice, SliceMut};
use crate::coordinate::world::SliceIndex;
use crate::{BlockPosition, ChunkPosition, SliceRange};

pub(crate) type SlabPointer = Box<Slab>;

pub struct ChunkTerrain {
    slabs: DoubleSidedVec<SlabPointer>,
    areas: Vec<Area>,
    boundary_links: Vec<(Area, Vec<BlockPosition>)>,
}

#[derive(Copy, Clone)]
pub enum SlabCreationPolicy {
    /// Don't add missing slabs
    PleaseDont,

    /// Create the missing slab and all intermediate slabs
    CreateAll,
}

impl ChunkTerrain {
    fn slabs_from_top(&self) -> impl Iterator<Item = &Slab> {
        self.slabs.iter_decreasing().map(|ptr| ptr.deref())
    }

    fn slabs_from_bottom(&self) -> impl Iterator<Item = &Slab> {
        self.slabs.iter_increasing().map(|ptr| ptr.deref())
    }

    fn add_slab(&mut self, slab: SlabPointer) {
        let idx = slab.index();
        self.slabs.add(slab, idx);
    }

    /// Creates slabs up to and including target
    fn create_slabs_until(&mut self, target: SlabIndex) {
        self.slabs
            .fill_until(target, |idx| SlabPointer::new(Slab::empty(idx)));
    }

    fn slab_index_for_slice(slice: SliceIndex) -> SlabIndex {
        (slice.0 as f32 / SLAB_SIZE as f32).floor() as SlabIndex
    }

    fn slice_index_in_slab(slice: SliceIndex) -> SliceIndex {
        let SliceIndex(mut idx) = slice;
        idx %= SLAB_SIZE as i32; // cap at slab size
        idx = idx.abs(); // positive only
        SliceIndex(idx)
    }

    pub fn slab_count(&self) -> usize {
        self.slabs.len()
    }

    pub fn slice<S: Into<SliceIndex>>(&self, index: S) -> Option<Slice> {
        let index = index.into();
        let slab_idx = ChunkTerrain::slab_index_for_slice(index);
        self.slabs
            .get(slab_idx)
            .map(|ptr| ptr.slice(ChunkTerrain::slice_index_in_slab(index)))
    }

    pub fn slice_mut<S: Into<SliceIndex>>(&mut self, index: S) -> Option<SliceMut> {
        let index = index.into();
        let slab_idx = ChunkTerrain::slab_index_for_slice(index);
        self.slabs
            .get_mut(slab_idx)
            .map(|ptr| ptr.slice_mut(ChunkTerrain::slice_index_in_slab(index)))
    }

    /// Returns the range of slices in this terrain rounded to the nearest slab
    pub fn slice_bounds_as_slabs(&self) -> SliceRange {
        let mut slabs = self.slabs.indices_increasing();
        let bottom = slabs.next().unwrap_or(0);
        let top = slabs.last().unwrap_or(0) + 1;

        SliceRange::from_bounds(bottom * SLAB_SIZE as i32, top * SLAB_SIZE as i32)
    }

    pub fn slice_range(&self, range: SliceRange) -> Generator<(), (SliceIndex, Slice)> {
        Gn::new_scoped(move |mut s| {
            for slice in range
                .into_iter()
                .filter_map(|idx| self.slice(idx).map(|s| (idx, s)))
            {
                s.yield_(slice);
            }

            done!();
        })
    }

    pub fn slices_from_bottom(&self) -> impl Iterator<Item = (SliceIndex, Slice)> {
        self.slabs_from_bottom().flat_map(|slab| {
            (0..Slab::slice_count()).map(move |idx| (SliceIndex(idx), slab.slice(idx)))
        })
    }

    pub fn get_block<B: Into<BlockPosition>>(&self, pos: B) -> Option<Block> {
        let pos = pos.into();
        self.slice(pos.2).map(|slice| slice[pos])
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

    // TODO set_block trait to reuse in ChunkBuilder (#46)
    // TODO variation that will dynamically add slab?

    pub(crate) fn discover_areas(&mut self, chunk_pos: ChunkPosition) {
        // TODO reuse a buffer for each slab

        // per slab
        for idx in self.slabs.indices_increasing() {
            let slab_below = self.slabs
                .get(idx - 1)
                .map(|s| s.slice(SLAB_SIZE as i32 - 1).into());
            let slab = self.slabs.get_mut(idx).unwrap();

            // collect slab into local grid
            let mut discovery = AreaDiscovery::from_slab(slab, slab_below);

            // flood fill and assign areas
            let area_count = discovery.flood_fill_areas();
            debug!(
                "slab {}: {} areas: {:?}",
                idx,
                area_count,
                discovery.areas()
            );

            // collect areas
            self.areas.extend(
                discovery
                    .areas()
                    .iter()
                    .map(|slab_area| slab_area.into_area(chunk_pos)),
            );

            // TODO discover internal area links

            // collect boundary areas and linking blocks

            discovery.apply(slab);
        }
    }

    /// Populates `out` with areas and their linking blocks
    pub(crate) fn areas_for_boundary(
        &self,
        boundary: ChunkBoundary,
        out: &mut HashMap<SlabArea, Vec<BlockPosition>>,
    ) {
        for slab in self.slabs.iter_increasing() {
            let idx = slab.index();

            for (slab_area, links) in boundary
                .blocks_in_slab(slab.index())
                .map(|pos| (pos, self.get_block(pos).unwrap().area_index()))
                .filter(|(_, a)| a.initialized())
                .group_by(|(_, a)| *a)
                .into_iter()
                .map(|(area, blocks)| {
                    let links = blocks.map(|(pos, _)| pos);

                    // promote slab-local area to chunk-local area
                    let slab_area = SlabArea { slab: idx, area };

                    (slab_area, links)
                }) {
                out.insert(slab_area, links.collect());
            }
        }
    }

    pub(crate) fn areas(&self) -> &[Area] {
        &self.areas
    }

    /// Only for tests
    #[cfg(test)]
    pub fn blocks<'a>(
        &self,
        out: &'a mut Vec<(BlockPosition, Block)>,
    ) -> &'a mut Vec<(BlockPosition, Block)> {
        use crate::chunk::{BLOCK_COUNT_SLICE, CHUNK_SIZE};

        let bottom_slab = self.slabs_from_bottom().next().unwrap();

        let low_z = bottom_slab.index() * SLAB_SIZE as i32;
        let high_z = low_z + (self.slab_count() * SLAB_SIZE) as i32;

        let total_size: usize = ((high_z - low_z) * BLOCK_COUNT_SLICE as i32) as usize;
        out.reserve(total_size);
        out.clear();

        let iter_from = if low_z != 0 { low_z + 1 } else { low_z };

        for z in iter_from..high_z {
            for y in 0..CHUNK_SIZE.as_u16() {
                for x in 0..CHUNK_SIZE.as_u16() {
                    let pos: BlockPosition = (x, y, z).into();
                    let block = self.get_block(pos);
                    out.push((pos, block.unwrap()));
                }
            }
        }

        out
    }
}

impl Default for ChunkTerrain {
    /// has single empty slab at index 0
    fn default() -> Self {
        let mut terrain = Self {
            slabs: DoubleSidedVec::with_capacity(8),
            areas: Vec::new(),
            boundary_links: Vec::new(),
        };

        terrain.add_slab(SlabPointer::new(Slab::empty(0)));

        terrain
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::slab::{Slab, SLAB_SIZE};
    use crate::chunk::terrain::{ChunkTerrain, SlabPointer};
    use crate::coordinate::world::SliceIndex;

    use super::*;

    #[test]
    fn empty() {
        let terrain = ChunkTerrain::default();
        assert_eq!(terrain.slab_count(), 1);
    }

    #[test]
    #[should_panic]
    fn no_dupes() {
        let mut terrain = ChunkTerrain::default();
        terrain.add_slab(SlabPointer::new(Slab::empty(0)));
    }

    #[test]
    fn slabs() {
        let mut terrain = ChunkTerrain::default();

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
        assert_eq!(ChunkTerrain::slab_index_for_slice(SliceIndex(4)), 0);
        assert_eq!(ChunkTerrain::slab_index_for_slice(SliceIndex(0)), 0);
        assert_eq!(ChunkTerrain::slab_index_for_slice(SliceIndex(-3)), -1);
        assert_eq!(ChunkTerrain::slab_index_for_slice(SliceIndex(-20)), -1);
        assert_eq!(ChunkTerrain::slab_index_for_slice(SliceIndex(100)), 3);
    }

    #[test]
    fn block_views() {
        let mut terrain = ChunkTerrain::default();

        *terrain.slice_mut(0).unwrap()[(0, 0)].block_type_mut() = BlockType::Stone;
        assert_eq!(
            terrain.slice(0).unwrap()[(0, 0)].block_type(),
            BlockType::Stone
        );
        assert_eq!(
            terrain.slice(10).unwrap()[(0, 0)].block_type(),
            BlockType::Air
        );

        assert!(terrain.slice(SLAB_SIZE as i32).is_none());
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

        let mut terrain = ChunkTerrain::default();
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
    fn flood_fill_areas() {
        let mut terrain = ChunkTerrain::default();
        terrain.slice_mut(0).unwrap().fill(BlockType::Stone);

        terrain.discover_areas((0, 0).into());
    }

    #[test]
    fn slab_areas() {
        // slab with flat slice 0 should have 1 area
        let mut slab = Slab::empty(0);
        slab.slice_mut(0).fill(BlockType::Stone);

        let area_count = AreaDiscovery::from_slab(&slab, None).flood_fill_areas();
        assert_eq!(area_count, 1);

        // slab with 2 unconnected floors should have 2
        let mut slab = Slab::empty(0);
        slab.slice_mut(0).fill(BlockType::Stone);
        slab.slice_mut(5).fill(BlockType::Stone);

        let area_count = AreaDiscovery::from_slab(&slab, None).flood_fill_areas();
        assert_eq!(area_count, 2);
    }

    #[test]
    fn cross_slab_walkability() {
        // a slab whose top layer is solid should mean the slab above's z=0 is walkable

        let mut terrain = ChunkTerrain::default();
        terrain.add_slab(SlabPointer::new(Slab::empty(1))); // add upper slab

        // fill top layer of first slab
        terrain
            .slice_mut(SLAB_SIZE as i32 - 1)
            .unwrap()
            .fill(BlockType::Stone);

        terrain.discover_areas((0, 0).into());

        // TODO 1 area at z=0
        assert_eq!(terrain.areas.len(), 1);
    }

    #[test]
    fn create_slab() {
        // setting blocks in non-existent places should create a slab to fill it

        const SLAB_SIZE_I32: i32 = SLAB_SIZE as i32;
        let mut terrain = ChunkTerrain::default();

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
}
