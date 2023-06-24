use misc::*;

use unit::world::{
    BlockCoord, BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex,
    SlabLocation, SliceBlock, SliceIndex, WorldPoint, WorldPosition, WorldPositionRange,
};

use crate::chunk::slab::SliceNavArea;
use crate::chunk::slice::{Slice, SliceOwned};

use crate::chunk::slice_navmesh::{SlabVerticalSpace, SliceAreaIndex, SliceAreaIndexAllocator};
use crate::chunk::terrain::{NeighbourAreaHash, SlabNeighbour, SlabStorage, SlabVersion};
use crate::chunk::{SlabData, SparseGrid};
use crate::navigation::{BlockGraph, SlabAreaIndex};
use crate::navigationv2::{
    filter_border_areas, is_border, ChunkArea, NavRequirement, SlabArea, SlabNavGraph,
};
use crate::neighbour::NeighbourOffset;
use crate::world::LoadNotifier;
use crate::{BlockOcclusion, Slab, SliceRange, World, WorldArea, WorldAreaV2, WorldContext};
use parking_lot::RwLock;
use petgraph::visit::Walker;
use std::collections::HashMap;
use std::iter::once;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::time::Instant;

pub type ChunkId = u64;

#[derive(Copy, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct AreaInfo {
    pub height: u8,
    /// Inclusive
    pub range: ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
}

pub struct Chunk<C: WorldContext> {
    /// Unique for each chunk
    pos: ChunkLocation,

    slabs: SlabStorage<C>,

    /// Big info about each area not needed for nav graph
    areas: HashMap<ChunkArea, AreaInfo>,

    // TODO use lockfree DashMap?
    slab_progress: RwLock<HashMap<SlabIndex, SlabLoadingStatus>>,
    slab_notify: LoadNotifier,
}

#[derive(Clone, Debug)]
pub enum SlabLoadingStatus {
    /// Not available
    Unloaded,

    /// Has been requested
    Requested,

    /// Terrain is in chunk with isolated occlusion and vertical space (*missing bottom slice*).
    TerrainInWorld,

    /// Was in Done but bottom slab changed, currently processing the update
    Updating,

    /// Bottom slice of and nav has been provided by slab below, so now full nav areas
    /// are available. Internal area links are known, and SlabNavGraph is stored in the chunk
    DoneInIsolation,

    /// All neighbouring slabs are present and inter-slab links are known
    Done,
}

pub enum SlabAvailability {
    NotRequested,
    InProgress,
    /// Last modify time
    Present(Instant),
}

impl<C: WorldContext> Chunk<C> {
    pub fn empty_with_world(world: &World<C>, pos: impl Into<ChunkLocation>) -> Self {
        Self {
            pos: pos.into(),
            slabs: SlabStorage::default(),
            areas: HashMap::new(),
            slab_progress: RwLock::new(HashMap::new()),
            slab_notify: world.load_notifications(),
        }
    }

    /// Disconnected from any world's slab notifications
    #[cfg(test)]
    pub fn empty(pos: impl Into<ChunkLocation>) -> Self {
        Self {
            pos: pos.into(),
            slabs: SlabStorage::default(),
            areas: HashMap::new(),
            slab_progress: RwLock::new(HashMap::new()),
            slab_notify: LoadNotifier::default(),
        }
    }

    pub fn terrain(&self) -> &SlabStorage<C> {
        &self.slabs
    }
    pub fn terrain_mut(&mut self) -> &mut SlabStorage<C> {
        &mut self.slabs
    }

    #[inline]
    pub fn pos(&self) -> ChunkLocation {
        self.pos
    }

    pub fn id(&self) -> ChunkId {
        let ChunkLocation(x, y) = self.pos;
        (x as u64) << 32 | (y as u64)
    }

    #[deprecated]
    pub fn area_info_for_block(&self, block: BlockPosition) -> Option<(ChunkArea, AreaInfo)> {
        let b = self.slabs.get_block(block)?;
        let area = b.nav_area()?;
        let chunk_area = ChunkArea {
            slab_idx: block.z().slab_index(),
            slab_area: SlabArea {
                slice_idx: block.z().to_local(),
                slice_area: area,
            },
        };
        // self.areas.get(&chunk_area).map(|ai| (chunk_area, *ai))
        unreachable!()
    }

    pub fn area_info(&self, slab: SlabIndex, slab_area: SlabArea) -> Option<AreaInfo> {
        self.areas
            .get(&ChunkArea {
                slab_idx: slab,
                slab_area,
            })
            .copied()
    }

    pub(crate) fn area_for_block(&self, block_pos: BlockPosition) -> Option<WorldArea> {
        self.slabs.get_block(block_pos).map(|b| {
            let area_index = b.area_index();
            WorldArea {
                chunk: self.pos,
                slab: block_pos.z().slab_index(),
                area: area_index,
            }
        })
    }

    pub fn iter_areas_with_info(&self) -> impl Iterator<Item = (ChunkArea, AreaInfo)> + '_ {
        self.areas.iter().map(|(k, v)| (*k, *v))
    }

    pub(crate) fn areas(&self) -> impl Iterator<Item = &ChunkArea> + ExactSizeIterator {
        self.areas.keys()
    }

    pub(crate) fn area_count(&self) -> usize {
        self.areas.len()
    }

    pub(crate) fn remove_block_graphs(&mut self, (min, max): (SlabIndex, SlabIndex)) {
        unreachable!()
    }

    pub(crate) fn block_graph_for_area(&self, area: WorldArea) -> Option<&BlockGraph> {
        unreachable!()
    }

    pub fn replace_all_slice_areas(&mut self, slab: SlabIndex, new_areas: &[SliceNavArea]) {
        // remove all for this slab
        self.areas.retain(|a, _| a.slab_idx != slab);

        let mut area_alloc = SliceAreaIndexAllocator::default();
        self.areas.extend(new_areas.iter().map(|a| {
            (
                ChunkArea {
                    slab_idx: slab,
                    slab_area: SlabArea {
                        slice_idx: a.slice,
                        slice_area: area_alloc.allocate(a.slice.slice()),
                    },
                },
                AreaInfo {
                    height: a.height,
                    range: (a.from, a.to),
                },
            )
        }));
    }

    pub fn slice_range(
        &self,
        range: SliceRange,
    ) -> impl Iterator<Item = (GlobalSliceIndex, Slice<C>)> {
        range
            .as_range()
            .map(move |i| self.slabs.slice(i).map(|s| (GlobalSliceIndex::new(i), s)))
            .skip_while(|s| s.is_none())
            .while_some()
    }

    pub fn slice_or_dummy(&self, slice: GlobalSliceIndex) -> Slice<C> {
        #[allow(clippy::redundant_closure)]
        self.slabs.slice(slice).unwrap_or_else(|| Slice::dummy())
    }

    pub(crate) fn mark_slab_requested(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::Requested);
        // no notification necessary, nothing waits for a slab to be requested
    }

    pub(crate) fn mark_slabs_requested(&self, slabs: impl Iterator<Item = SlabIndex>) {
        self.update_slabs_status(slabs, SlabLoadingStatus::Requested);
        // no notification necessary, nothing waits for a slab to be requested
    }

    pub(crate) fn mark_slab_as_in_world(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::TerrainInWorld);
        self.slab_notify.notify(SlabLocation::new(slab, self.pos));
    }

    fn mark_slab_as_done_in_isolation(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::DoneInIsolation);
        self.slab_notify.notify(SlabLocation::new(slab, self.pos));
    }

    pub(crate) fn mark_slab_as_updating(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::Updating);
        // self.slab_notify.notify(SlabLocation::new(slab, self.pos));
    }

    pub(crate) fn mark_slab_as_done(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::Done);
        self.slab_notify.notify(SlabLocation::new(slab, self.pos));
    }

    fn update_slab_status(&self, slab: SlabIndex, state: SlabLoadingStatus) {
        trace!("updating slab progress"; SlabLocation::new(slab, self.pos), "state" => ?state);
        self.slab_progress.write().insert(slab, state);
    }

    /// Does not notify
    fn update_slabs_status(
        &self,
        mut slabs: impl Iterator<Item = SlabIndex>,
        state: SlabLoadingStatus,
    ) {
        let mut map = self.slab_progress.write();
        for slab in slabs {
            trace!("updating slab progress"; SlabLocation::new(slab, self.pos), "state" => ?state);
            map.insert(slab, state.clone());
        }
    }

    fn slab_progress(&self, slab: SlabIndex) -> SlabLoadingStatus {
        let guard = self.slab_progress.read();
        guard
            .get(&slab)
            .unwrap_or(&SlabLoadingStatus::Unloaded)
            .clone()
    }

    /// Returns true if slab has not been already requested/loaded or is a placeholder
    pub fn should_slab_be_loaded(&self, slab: SlabIndex) -> bool {
        match self.slab_progress(slab) {
            SlabLoadingStatus::Unloaded => {
                // has not been requested, pls load
                true
            }
            _ => false, // is currently in progress or done, don't request again
        }
    }

    pub fn iter_loading_slabs(&self) -> impl Iterator<Item = (SlabIndex, SlabLoadingStatus)> + '_ {
        self.iter_slabs_filtered(|s| {
            !matches!(s, SlabLoadingStatus::Unloaded | SlabLoadingStatus::Done)
        })
    }

    /// All states except unloaded
    pub fn iter_slabs(&self) -> impl Iterator<Item = (SlabIndex, SlabLoadingStatus)> + '_ {
        self.iter_slabs_filtered(|s| !matches!(s, SlabLoadingStatus::Unloaded))
    }

    /// Allocates :(
    fn iter_slabs_filtered(
        &self,
        filter: impl Fn(&SlabLoadingStatus) -> bool,
    ) -> impl Iterator<Item = (SlabIndex, SlabLoadingStatus)> + '_ {
        let guard = self.slab_progress.read();
        let vec = guard
            .iter()
            .filter_map(|(slab, state)| filter(state).then(|| (*slab, state.clone())))
            .collect_vec();
        vec.into_iter()
    }

    pub fn is_slab_loaded(&self, slab: SlabIndex) -> bool {
        let progress = self.slab_progress(slab);
        matches!(progress, SlabLoadingStatus::Done)
    }

    pub fn slab_vertical_space_or_wait(
        &self,
        slab: SlabIndex,
    ) -> SlabThingOrWait<Arc<SlabVerticalSpace>> {
        use SlabLoadingStatus::*;
        use SlabThingOrWait::*;
        let progress = self.slab_progress(slab);

        match progress {
            Unloaded => Failure,
            Requested | Updating => Wait,
            _ => Ready(self.slab_vertical_space(slab).unwrap()),
        }
    }

    pub fn get_slab_or_wait(&self, slab: SlabIndex) -> SlabThingOrWait<&Slab<C>> {
        use SlabLoadingStatus::*;
        use SlabThingOrWait::*;
        let progress = self.slab_progress(slab);

        match progress {
            Unloaded => Failure,
            Requested | Updating => Wait,
            TerrainInWorld | DoneInIsolation | Done => Ready(self.slabs.slab(slab).unwrap()),
        }
    }

    pub fn get_slab_areas_or_wait(
        &self,
        slab: SlabIndex,
        mut func: impl FnMut(&ChunkArea, &AreaInfo),
    ) -> SlabThingOrWait<()> {
        use SlabLoadingStatus::*;
        use SlabThingOrWait::*;

        let progress = self.slab_progress(slab);

        match progress {
            Unloaded => Failure,
            Requested | TerrainInWorld | Updating => Wait,
            DoneInIsolation | Done => {
                self.areas
                    .iter()
                    .filter(|(area, info)| area.slab_idx == slab)
                    .for_each(|(a, i)| func(a, i));
                Ready(())
            }
        }
    }

    pub fn has_slab(&self, slab: SlabIndex) -> bool {
        self.slabs.slab(slab).is_some()
    }

    pub fn slab_load_status(&self, slab: SlabIndex) -> impl Debug {
        self.slab_progress(slab)
    }

    pub fn slab_availability(&self, slab: SlabIndex) -> SlabAvailability {
        use SlabLoadingStatus::*;
        match self.slab_progress(slab) {
            Unloaded => SlabAvailability::NotRequested,
            Requested | TerrainInWorld | DoneInIsolation | Updating => SlabAvailability::InProgress,
            Done => {
                let data = self
                    .terrain()
                    .slab_data(slab)
                    .expect("slab data should be present for Done slab");
                SlabAvailability::Present(data.last_modify_time)
            }
        }
    }

    pub fn slab_vertical_space(&self, slab: SlabIndex) -> Option<Arc<SlabVerticalSpace>> {
        self.slabs.slab_data(slab).map(|s| s.vertical_space.clone())
    }

    /// Sets state to DoneInIsolation. Returns prev neighbour hashes
    pub(crate) fn replace_slab_nav_graph(
        &mut self,
        slab: SlabIndex,
        graph: SlabNavGraph,
        areas: &[SliceNavArea],
    ) -> Option<[NeighbourAreaHash; 6]> {
        if let Some(s) = self.slabs.slab_data_mut(slab) {
            s.nav = graph;

            let old_hashes = s.neighbour_edge_hashes;

            // calculate hashes for border areas
            for (n_dir, out) in SlabNeighbour::VALUES
                .iter()
                .zip(s.neighbour_edge_hashes.iter_mut())
            {
                *out = NeighbourAreaHash::for_areas_with_edge(*n_dir, areas.iter().copied());
                trace!(
                    "new neighbour hash for {:?}: {:?} ({} neighbours)",
                    n_dir,
                    *out,
                    areas.iter().filter(|a| n_dir.is_border(*a)).count()
                )
            }

            self.mark_slab_as_done_in_isolation(slab);
            return Some(old_hashes);
        }

        None
    }

    pub(crate) fn update_slab_terrain_derived_data(
        &mut self,
        slab: SlabIndex,
        vs: Arc<SlabVerticalSpace>,
        occlusion: SparseGrid<BlockOcclusion>,
    ) {
        if let Some(s) = self.slabs.slab_data_mut(slab) {
            s.vertical_space = vs;
            s.occlusion = occlusion;
        }
    }

    pub fn slab_nav_graph(&self, slab: SlabIndex) -> Option<&SlabNavGraph> {
        self.slabs.slab_data(slab).map(|s| &s.nav)
    }

    /// Searches downwards in vertical space for area z
    pub fn find_area_for_block_with_height(
        &self,
        block: BlockPosition,
        requirement: NavRequirement,
    ) -> Option<(SlabArea, AreaInfo)> {
        let slab_idx = SlabIndex::from(block.z());
        let slab = self.slabs.slab_data(slab_idx)?;
        let area_slice_idx = slab.vertical_space.find_slice(block.into())?;

        // find matching area in graph (bounds checks all areas in that slice... might be fine)
        let slice_block = SliceBlock::from(BlockPosition::from(block));
        slab.nav.iter_nodes().find_map(move |a| {
            if a.slice_idx == area_slice_idx {
                // bounds check
                let info = self
                    .area_info(slab_idx, a)
                    .unwrap_or_else(|| panic!("unknown area {a:?} in chunk {:?}", self.pos));
                if info.fits_requirement(requirement) && info.contains(slice_block) {
                    return Some((a, info));
                }
            }

            None
        })
    }
}

impl AreaInfo {
    pub fn contains(&self, block: SliceBlock) -> bool {
        let (x, y) = block.xy();
        let ((x1, y1), (x2, y2)) = self.range;
        x >= x1 && x <= x2 && y >= y1 && y <= y2
    }

    pub fn centre_pos(&self, area: crate::navigationv2::world_graph::WorldArea) -> WorldPosition {
        let (range_min, range_max) = self.range;
        let centre_x = (range_min.0 + range_max.0) / 2;
        let centre_y = (range_min.1 + range_max.1) / 2;
        Self::pos_to_world(area, (centre_x, centre_y))
    }

    pub fn min_pos(&self, area: crate::navigationv2::world_graph::WorldArea) -> WorldPosition {
        Self::pos_to_world(area, self.range.0)
    }

    pub fn max_pos(&self, area: crate::navigationv2::world_graph::WorldArea) -> WorldPosition {
        Self::pos_to_world(area, self.range.1)
    }

    /// Inclusive size
    pub fn size(&self) -> (u8, u8) {
        (
            self.range.1 .0 - self.range.0 .0 + 1,
            self.range.1 .1 - self.range.0 .1 + 1,
        )
    }

    pub fn borders(&self) -> ArrayVec<NeighbourOffset, 4> {
        NeighbourOffset::aligned()
            .filter_map(|(d, _)| is_border(d, self.range).then_some(d))
            .collect()
    }

    /// Point is relative to this area
    pub fn random_point(&self, max_xy: u8, random: &mut dyn RngCore) -> (f32, f32) {
        debug_assert!(self.fits_xy(max_xy));
        let ((xmin, ymin), (xmax, ymax)) = self.range;
        let half_width = max_xy as f32 * 0.5;
        (
            random.gen_range(xmin as f32 + half_width, xmax as f32 - half_width + 1.0) - half_width,
            random.gen_range(ymin as f32 + half_width, ymax as f32 - half_width + 1.0) - half_width,
        )
    }

    pub fn as_range(&self, area: WorldAreaV2) -> WorldPositionRange {
        WorldPositionRange::with_inclusive_range(self.min_pos(area), self.max_pos(area))
    }

    pub fn random_world_point(
        &self,
        max_xy: u8,
        slice: GlobalSliceIndex,
        chunk: ChunkLocation,
        random: &mut dyn RngCore,
    ) -> WorldPoint {
        let (x, y) = self.random_point(max_xy, random);

        // TODO new BlockPoint for BlockPosition but floats. this conversion is gross
        let block_pos = BlockPosition::new_unchecked(x as BlockCoord, y as BlockCoord, slice);
        let world_pos = block_pos.to_world_position(chunk).floored();

        world_pos + (x.fract(), y.fract(), 0.0)
    }

    pub fn fits_xy(&self, max_xy: u8) -> bool {
        let (x, y) = self.size();
        max_xy < x.min(y)
    }

    pub fn fits_requirement(&self, req: NavRequirement) -> bool {
        self.height >= req.height && self.fits_xy(req.max_xy)
    }

    fn pos_to_world(
        area: crate::navigationv2::world_graph::WorldArea,
        (x, y): (BlockCoord, BlockCoord),
    ) -> WorldPosition {
        BlockPosition::new_unchecked(
            x,
            y,
            area.chunk_area
                .slab_area
                .slice_idx
                .to_global(area.chunk_area.slab_idx),
        )
        .to_world_position(area.chunk_idx)
    }

    pub(crate) fn as_slice_nav_area(&self, slice: LocalSliceIndex) -> SliceNavArea {
        SliceNavArea {
            slice,
            from: self.range.0,
            to: self.range.1,
            height: self.height,
        }
    }
}

pub enum SlabThingOrWait<T> {
    Wait,
    Failure,
    Ready(T),
}

#[cfg(test)]
mod tests {
    use unit::world::GlobalSliceIndex;

    use crate::chunk::{Chunk, ChunkBuilder};
    use crate::helpers::{DummyBlockType, DummyWorldContext};
    use unit::world::CHUNK_SIZE;

    #[test]
    fn chunk_ops() {
        // check setting and getting blocks works
        let chunk = ChunkBuilder::<DummyWorldContext>::new()
            .apply(|c| {
                // a bit on slice 0
                for i in 0..3 {
                    c.set_block((i, i, 0), DummyBlockType::Dirt);
                }
            })
            .set_block((2, 3, 1), DummyBlockType::Dirt)
            .into_inner();

        // slice 1 was filled
        assert_eq!(
            chunk.get_block_tup((2, 3, 1)).map(|b| b.block_type()),
            Some(DummyBlockType::Dirt)
        );

        // collect slice
        let slice: Vec<DummyBlockType> = chunk
            .slice(GlobalSliceIndex::new(0))
            .unwrap()
            .iter()
            .map(|b| b.block_type())
            .collect();
        assert_eq!(slice.len(), CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize()); // ensure exact length
        assert_eq!(
            slice.iter().filter(|b| **b != DummyBlockType::Air).count(),
            3
        ); // ensure exact number of filled blocks

        // ensure each exact coord was filled
        assert_eq!(
            chunk.get_block_tup((0, 0, 0)).map(|b| b.block_type()),
            Some(DummyBlockType::Dirt)
        );
        assert_eq!(
            chunk.get_block_tup((1, 1, 0)).map(|b| b.block_type()),
            Some(DummyBlockType::Dirt)
        );
        assert_eq!(
            chunk.get_block_tup((2, 2, 0)).map(|b| b.block_type()),
            Some(DummyBlockType::Dirt)
        );
    }

    #[test]
    fn chunk_id() {
        // check chunk ids are unique
        let id1 = Chunk::<DummyWorldContext>::empty((0, 0)).id();
        let id2 = Chunk::<DummyWorldContext>::empty((0, 1)).id();
        let id3 = Chunk::<DummyWorldContext>::empty((1, 0)).id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn blocks() {
        // check individual block collection is ordered as intended
        let c = Chunk::<DummyWorldContext>::empty((0, 0));
        let mut blocks = Vec::new();
        c.slabs.blocks(&mut blocks);
        let mut b = blocks.into_iter();
        assert_eq!(
            b.next().map(|(p, b)| (p.xyz(), b.block_type())),
            Some(((0, 0, 0.into()), DummyBlockType::Air))
        );
        assert_eq!(
            b.next().map(|(p, b)| (p.xyz(), b.block_type())),
            Some(((1, 0, 0.into()), DummyBlockType::Air))
        );
        assert_eq!(
            b.next().map(|(p, b)| (p.xyz(), b.block_type())),
            Some(((2, 0, 0.into()), DummyBlockType::Air))
        );
    }
}
