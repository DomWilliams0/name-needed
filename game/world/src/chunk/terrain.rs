use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use common::*;

use crate::area::discovery::AreaDiscovery;
use crate::area::{BlockGraph, ChunkArea, ChunkBoundary, WorldArea};
use crate::block::Block;
use crate::chunk::double_sided_vec::DoubleSidedVec;
use crate::chunk::slab::{Slab, SlabIndex, SLAB_SIZE};
use crate::chunk::slice::{unflatten_index, Slice, SliceMut};
use crate::occlusion::{BlockOcclusion, NeighbourOffset};
use crate::SliceRange;
use std::iter::once;
use unit::world::{BlockPosition, ChunkPosition, SliceIndex};

pub(crate) type SlabPointer = Box<Slab>;

pub struct ChunkTerrain {
    slabs: DoubleSidedVec<SlabPointer>,
    areas: HashMap<WorldArea, BlockGraph>,
    boundary_links: Vec<(WorldArea, Vec<BlockPosition>)>,
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

    /// Creates slabs up to and including target
    fn create_slabs_until(&mut self, target: SlabIndex) {
        self.slabs
            .fill_until(target, |idx| SlabPointer::new(Slab::empty(idx)));
    }

    pub(crate) fn slab_index_for_slice(slice: SliceIndex) -> SlabIndex {
        (slice.0 as f32 / SLAB_SIZE.as_f32()).floor() as SlabIndex
    }

    fn slice_index_in_slab(slice: SliceIndex) -> SliceIndex {
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

        SliceRange::from_bounds(bottom * SLAB_SIZE.as_i32(), top * SLAB_SIZE.as_i32())
    }

    pub fn slice_range(&self, range: SliceRange) -> impl Iterator<Item = Slice> {
        range.as_range().map(move |i| self.slice(i)).while_some()
    }

    pub fn slices_from_bottom(&self) -> impl Iterator<Item = (SliceIndex, Slice)> {
        self.slabs_from_bottom().flat_map(|slab| {
            (0..Slab::slice_count()).map(move |idx| (SliceIndex(idx), slab.slice(idx)))
        })
    }

    pub fn slices_from_bottom_mut_foreach<F: Fn(SliceIndex, SliceMut)>(&mut self, f: F) {
        for (slab_idx, slab) in self.slabs_from_bottom_mut() {
            let base_slice_idx = SLAB_SIZE.as_i32() * slab_idx;
            for slice_idx in 0..Slab::slice_count() {
                f(
                    SliceIndex(base_slice_idx + slice_idx),
                    slab.slice_mut(slice_idx),
                )
            }
        }
    }

    pub fn slices_from_top(&self) -> impl Iterator<Item = (SliceIndex, Slice)> {
        self.slabs_from_top().flat_map(|slab| {
            (0..Slab::slice_count())
                .rev()
                .map(move |idx| (SliceIndex(idx), slab.slice(idx)))
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
            let slice_below = self
                .slabs
                .get(idx - 1)
                .map(|s| s.slice(SLAB_SIZE.as_i32() - 1).into());
            let slice_above = self.slabs.get(idx + 1).map(|s| s.slice(0).into());
            let slab = self.slabs.get_mut(idx).unwrap();

            // collect slab into local grid
            let mut discovery = AreaDiscovery::from_slab(slab, slice_below, slice_above);

            // flood fill and assign areas
            let area_count = discovery.flood_fill_areas();
            debug!("slab {}: {} areas", idx, area_count);

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

    /// Populates `out` with areas and their linking blocks
    pub(crate) fn areas_for_boundary(
        &self,
        boundary: ChunkBoundary,
        out: &mut HashMap<ChunkArea, Vec<BlockPosition>>,
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
                    let chunk_area = ChunkArea { slab: idx, area };

                    (chunk_area, links)
                })
            {
                out.insert(slab_area, links.collect());
            }
        }
    }

    pub(crate) fn areas(&self) -> impl Iterator<Item = &WorldArea> {
        self.areas.keys()
    }

    pub(crate) fn block_graph_for_area(&self, area: WorldArea) -> Option<&BlockGraph> {
        self.areas.get(&area)
    }

    /// Only for tests
    #[cfg(test)]
    pub fn blocks<'a>(
        &self,
        out: &'a mut Vec<(BlockPosition, Block)>,
    ) -> &'a mut Vec<(BlockPosition, Block)> {
        use crate::chunk::{BLOCK_COUNT_SLICE, CHUNK_SIZE};

        let bottom_slab = self.slabs_from_bottom().next().unwrap();

        let low_z = bottom_slab.index() * SLAB_SIZE.as_i32();
        let high_z = low_z + (self.slab_count() * SLAB_SIZE.as_usize()) as i32;

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
    // (slab0 slice0 mut, slab0 slice1 immut), (slab0 slice1 mut, slab0 slice2 immut) ...
    // ... (slab0 sliceN mut, slab1 slice0), (slab1 slice0 mut, slab1 slice1) ...
    // ... (slabN sliceN-1 mut, slabN sliceN)
    pub fn ascending_slice_pairs_foreach<F: FnMut(SliceMut, Slice)>(&mut self, mut f: F) {
        // need to include a null slab at the end so the last slab is iterated too
        let indices = self
            .slabs
            .indices_increasing()
            .map(Some)
            .chain(once(None))
            .tuple_windows();

        for (this_slab_idx, next_slab_idx) in indices {
            let this_slab_idx = this_slab_idx.unwrap(); // first slab is always Some

            let this_slab = self.slabs.get_mut(this_slab_idx).unwrap();

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
                    // can't have a mut and immut ref to self.slabs
                    let mut slice = this_slab.slice_mut(Slab::slice_count() - 1);
                    let ptr = slice.as_mut_ptr();
                    SliceMut::from_ptr(ptr)
                };

                let next_slab_bottom_slice = self.slabs.get(next_slab_idx).unwrap().slice(0);
                f(this_slab_top_slice, next_slab_bottom_slice);
            }
        }
    }

    pub fn init_occlusion(&mut self) {
        self.ascending_slice_pairs_foreach(|mut slice_this, slice_next| {
            for (i, b) in slice_this
                .iter_mut()
                .enumerate()
                // this block should be solid
                .filter(|(_, b)| b.solid())
                // and the one above it should not be
                .filter(|(i, _)| !(*slice_next)[*i].solid())
            {
                let this_block = unflatten_index(i);

                // collect blocked state of each neighbour on the top face
                let mut blocked = [false; 8];
                for (n, offset) in NeighbourOffset::offsets() {
                    if let Some(neighbour_block) = this_block.try_add(offset) {
                        blocked[n as usize] = slice_next[neighbour_block].solid();
                    }
                }

                *b.occlusion_mut() = BlockOcclusion::from_neighbour_opacities(blocked);
            }
        });
    }
}

impl Default for ChunkTerrain {
    /// has single empty slab at index 0
    fn default() -> Self {
        let mut terrain = Self {
            slabs: DoubleSidedVec::with_capacity(8),
            areas: HashMap::new(),
            boundary_links: Vec::new(),
        };

        terrain.add_slab(SlabPointer::new(Slab::empty(0)));

        terrain
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;
    use ordered_float::OrderedFloat;
    use petgraph::visit::{IntoNodeReferences, NodeRef};
    use petgraph::Direction;

    use crate::area::EdgeCost;
    use crate::block::{BlockHeight, BlockType};
    use crate::chunk::slab::{Slab, SLAB_SIZE};
    use crate::chunk::terrain::{ChunkTerrain, SlabPointer};
    use unit::world::SliceIndex;

    use super::*;
    use crate::occlusion::VertexOcclusion;

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
    fn slab_areas_step() {
        // terrain with accessible half steps should still be 1 area

        let mut terrain = ChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll); // solid walkable

        // half steps next to it
        terrain.set_block(
            (3, 2, 3),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        );
        terrain.set_block(
            (1, 2, 3),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        );

        // 1 area still
        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 1);

        // half step out of reach is still unreachable
        let mut terrain = ChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block(
            (4, 2, 3),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        );

        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 2);
    }

    #[test]
    fn slab_areas_jump() {
        // terrain with accessible jumps should still be 1 area

        let mut terrain = ChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll); // solid walkable

        // full jump staircase next to it
        terrain.set_block((3, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((4, 2, 4), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((5, 2, 4), BlockType::Stone, SlabCreationPolicy::CreateAll);

        // 1 area still
        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 1);

        // too big jump out of reach is still unreachable
        let mut terrain = ChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((3, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((4, 2, 7), BlockType::Stone, SlabCreationPolicy::CreateAll);

        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 2);

        // if above is blocked, can't jump
        let mut terrain = ChunkTerrain::default();
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((3, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((2, 2, 4), BlockType::Stone, SlabCreationPolicy::CreateAll); // blocks jump!

        // so 2 areas expected
        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 2);
    }

    #[test]
    fn cross_slab_walkability() {
        // a slab whose top layer is solid should mean the slab above's z=0 is walkable

        let mut terrain = ChunkTerrain::default();
        terrain.add_slab(SlabPointer::new(Slab::empty(1))); // add upper slab

        // fill top layer of first slab
        terrain
            .slice_mut(SLAB_SIZE.as_i32() - 1)
            .unwrap()
            .fill(BlockType::Stone);

        terrain.discover_areas((0, 0).into());

        // TODO 1 area at z=0
        assert_eq!(terrain.areas.len(), 1);
    }

    #[test]
    fn create_slab() {
        // setting blocks in non-existent places should create a slab to fill it

        const SLAB_SIZE_I32: i32 = SLAB_SIZE.as_i32();
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

    #[test]
    fn block_graph_step_up() {
        // a half step up should be a valid edge
        let mut terrain = ChunkTerrain::default();
        terrain.set_block(
            (2, 2, 2),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        );
        terrain.set_block((3, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);

        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 1);

        let graph = terrain.areas.values().next().unwrap();
        assert_eq!(graph.graph().node_count(), 2);
        assert_eq!(graph.graph().edge_count(), 2);
    }

    #[test]
    fn block_graph_long_route() {
        // check that both ways up and down a staircase of jumps and half steps work as intended

        let mut terrain = ChunkTerrain::default();
        // 3 flat
        terrain.set_block((2, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((3, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((4, 2, 2), BlockType::Stone, SlabCreationPolicy::CreateAll);

        // 2 half steps of the same height
        terrain.set_block(
            (5, 2, 3),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        );
        terrain.set_block(
            (6, 2, 3),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        );

        // 1 jump up to another half step
        terrain.set_block(
            (7, 2, 4),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        );

        // 1 half step up to full
        terrain.set_block((8, 2, 4), BlockType::Stone, SlabCreationPolicy::CreateAll);

        // 2 jumps up
        terrain.set_block((9, 2, 5), BlockType::Stone, SlabCreationPolicy::CreateAll);
        terrain.set_block((10, 2, 6), BlockType::Stone, SlabCreationPolicy::CreateAll);

        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 1);

        let graph = terrain.areas.values().next().unwrap();
        assert_eq!(graph.graph().node_count(), 9);

        // ------ upwards
        let path = graph
            .find_path((2, 2, 3), (10, 2, 7))
            .expect("path should succeed")
            .as_tuples();
        assert_eq!(path.len(), 9);

        // collect edge pairs
        let edges = path
            .iter()
            .tuple_windows()
            .map(|(&a, &b)| graph.get_edge_between(a, b).unwrap())
            .collect_vec();

        assert_eq!(
            edges,
            vec![
                // flat walking
                EdgeCost::Walk,
                EdgeCost::Walk,
                // step up half step
                EdgeCost::Step(OrderedFloat(0.5)),
                // half step to half step
                EdgeCost::Walk,
                // jump up to half step
                EdgeCost::JumpUp,
                // step up half step again
                EdgeCost::Step(OrderedFloat(0.5)),
                // jumps
                EdgeCost::JumpUp,
                EdgeCost::JumpUp,
            ]
        );

        // ------ downwards
        let path = graph
            .find_path((10, 2, 7), (2, 2, 3))
            .expect("reverse path should succeed")
            .as_tuples();
        assert_eq!(path.len(), 9);

        // collect edge pairs
        let edges = path
            .iter()
            .tuple_windows()
            .map(|(&a, &b)| graph.get_edge_between(a, b).unwrap())
            .collect_vec();

        assert_eq!(
            edges,
            vec![
                // jump down twice
                EdgeCost::JumpDown,
                EdgeCost::JumpDown,
                // step down
                EdgeCost::Step(OrderedFloat(-0.5)),
                // jump down to next half step
                EdgeCost::JumpDown,
                // walk to next half step on same level
                EdgeCost::Walk,
                // step down
                EdgeCost::Step(OrderedFloat(-0.5)),
                // flat walking
                EdgeCost::Walk,
                EdgeCost::Walk,
            ]
        );
    }

    #[test]
    fn block_graph_high_jump() {
        // there should be no edge that is a jump of > 1.0

        let mut terrain = ChunkTerrain::default();
        terrain.set_block(
            (2, 2, 2),
            (BlockType::Stone, BlockHeight::Half),
            SlabCreationPolicy::CreateAll,
        ); // half block
        terrain.set_block((3, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll); // technically a vertical neighbour but the jump is too high

        terrain.discover_areas((0, 0).into());
        assert_eq!(terrain.areas.len(), 2); // 2 disconnected areas
    }

    #[test]
    fn discovery_neighbour_count() {
        let mut terrain = ChunkTerrain::default();
        terrain.slice_mut(1).unwrap().fill(BlockType::Stone);
        terrain.discover_areas((0, 0).into());

        // check random block in the middle
        let (_area, block_graph) = terrain.areas.iter().next().unwrap();
        let (idx, _node) = block_graph
            .graph()
            .node_references()
            .find(|n| n.weight().0 == (4, 4, 2).into())
            .unwrap();

        assert_matches!(block_graph.get_edge_between((4, 4, 2), (4, 3, 2)), Some(_));
        assert_matches!(block_graph.get_edge_between((4, 4, 2), (4, 5, 2)), Some(_));
        assert_matches!(block_graph.get_edge_between((4, 4, 2), (3, 4, 2)), Some(_));
        assert_matches!(block_graph.get_edge_between((4, 4, 2), (5, 4, 2)), Some(_));

        assert_eq!(
            block_graph
                .graph()
                .edges_directed(idx, Direction::Outgoing)
                .count(),
            4
        );
        assert_eq!(
            block_graph
                .graph()
                .edges_directed(idx, Direction::Incoming)
                .count(),
            4
        );
    }

    #[test]
    fn slice_index_in_slab() {
        // positives are simple modulus
        assert_eq!(
            ChunkTerrain::slice_index_in_slab(SliceIndex(5)),
            SliceIndex(5)
        );
        assert_eq!(
            ChunkTerrain::slice_index_in_slab(SliceIndex(SLAB_SIZE.as_i32() + 4)),
            SliceIndex(4)
        );

        // negatives work backwards
        assert_eq!(
            ChunkTerrain::slice_index_in_slab(SliceIndex(-1)),
            SliceIndex(SLAB_SIZE.as_i32() - 1)
        );
    }

    #[test]
    fn occlusion_in_slab() {
        // no occlusion because the block directly above 2,2,2 is solid
        let mut terrain = ChunkTerrain::default();
        assert!(terrain.set_block((2, 2, 2), BlockType::Dirt, SlabCreationPolicy::CreateAll));
        assert!(terrain.set_block((2, 2, 3), BlockType::Stone, SlabCreationPolicy::CreateAll));
        assert!(terrain.set_block((2, 3, 3), BlockType::Dirt, SlabCreationPolicy::CreateAll));
        terrain.init_occlusion();

        let occlusion = *terrain.get_block((2, 2, 2)).unwrap().occlusion();
        assert_matches!(occlusion.corner(0), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(1), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(2), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(3), VertexOcclusion::NotAtAll);

        // occlusion will be populated if block directly above it is air
        assert!(terrain.set_block((2, 2, 3), BlockType::Air, SlabCreationPolicy::CreateAll));
        terrain.init_occlusion();

        let occlusion = *terrain.get_block((2, 2, 2)).unwrap().occlusion();
        assert_matches!(occlusion.corner(0), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(1), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(2), VertexOcclusion::Mildly);
        assert_matches!(occlusion.corner(3), VertexOcclusion::Mildly);
    }

    #[test]
    fn occlusion_across_slab() {
        let mut terrain = ChunkTerrain::default();
        assert!(terrain.set_block(
            (2, 2, SLAB_SIZE.as_i32() - 1),
            BlockType::Dirt,
            SlabCreationPolicy::CreateAll
        ));
        assert!(terrain.set_block(
            (2, 3, SLAB_SIZE.as_i32()),
            BlockType::Dirt,
            SlabCreationPolicy::CreateAll
        )); // next slab up
        terrain.init_occlusion();

        let occlusion = *terrain
            .get_block((2, 2, SLAB_SIZE.as_i32() - 1))
            .unwrap()
            .occlusion();
        assert_matches!(occlusion.corner(0), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(1), VertexOcclusion::NotAtAll);
        assert_matches!(occlusion.corner(2), VertexOcclusion::Mildly);
        assert_matches!(occlusion.corner(3), VertexOcclusion::Mildly);
    }

    #[test]
    fn ascending_slice_pairs() {
        let mut terrain = ChunkTerrain::default();

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

        assert_eq!(terrain.slab_count(), 3);
        const EXPECTED_COUNT: i32 = (SLAB_SIZE.as_i32() * 3) - 1;

        let mut count = 0;
        let mut expected = PATTERN.iter().cycle().copied().peekable();
        terrain.ascending_slice_pairs_foreach(|a, b| {
            count += 1;

            assert_eq!(dbg!(a[(0, 0)].block_type()), expected.next().unwrap());

            let expected_next = if count == EXPECTED_COUNT {
                BlockType::Air // top of highest slab with nothing above it
            } else {
                *expected.peek().unwrap()
            };

            assert_eq!(dbg!(b[(0, 0)].block_type()), expected_next);
        });

        assert_eq!(count, EXPECTED_COUNT);
    }
}
