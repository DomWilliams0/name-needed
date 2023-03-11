use misc::*;

use unit::world::{
    BlockCoord, BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex,
    SlabLocation, SliceIndex,
};

use crate::chunk::chunk::VerticalSpaceOrWait::Failure;
use crate::chunk::slab::{Slab, SliceNavArea};
use crate::chunk::slice::{Slice, SliceOwned};
use crate::chunk::slice_navmesh::{FreeVerticalSpace, VerticalSpacePlease};
use crate::chunk::slice_navmesh::{SlabVerticalSpace, SliceAreaIndexAllocator};
use crate::chunk::terrain::RawChunkTerrain;
use crate::chunk::BaseTerrain;
use crate::navigation::{BlockGraph, WorldArea};
use crate::navigationv2::{ChunkArea, SlabArea, SlabNavGraph};
use crate::world::LoadNotifier;
use crate::{SliceRange, World, WorldContext};
use parking_lot::RwLock;
use petgraph::visit::Walker;
use std::collections::HashMap;
use std::iter::once;
use std::ops::Deref;
use std::sync::{Arc, Weak};

pub type ChunkId = u64;

#[derive(Copy, Clone, Debug)]
pub struct AreaInfo {
    pub height: u8,
    /// Inclusive
    pub range: ((BlockCoord, BlockCoord), (BlockCoord, BlockCoord)),
}

pub struct Chunk<C: WorldContext> {
    /// Unique for each chunk
    pos: ChunkLocation,

    terrain: RawChunkTerrain<C>,

    /// Sparse associated data with each block. Unused atm?
    block_data: HashMap<BlockPosition, C::AssociatedBlockData>,

    /// Big info about each area not needed for nav graph
    areas: HashMap<ChunkArea, AreaInfo>,

    // TODO instead of multiple bad hashmaps 1:1 for each slabindex, store inline alongside slabs in RawChunkTerrain. smae with vertical_space
    slab_navs: HashMap<SlabIndex, SlabNavGraph>,

    vertical_space: HashMap<SlabIndex, SlabVerticalSpace>,

    slab_progress: RwLock<HashMap<SlabIndex, SlabLoadingStatus>>,
    slab_notify: LoadNotifier,
}

#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""))]
pub(crate) enum SlabLoadingStatus {
    /// Not available
    Unloaded,

    /// Has been requested
    Requested,

    /// Terrain is in chunk with isolated occlusion and vertical space and nav areas (both are *missing bottom slice*).
    /// Depends on slab above
    TerrainInWorld,

    /// Bottom slice of and nav has been provided by slab below, so now full nav areas
    /// are available. Internal area links are known
    DoneInIsolation,

    /// All neighbouring slabs are present and inter-slab links are known
    Done,
}

enum SlabAvailability<C: WorldContext> {
    Never,
    OnItsWay,
    TadaItsAvailable(SliceOwned<C>),
}

impl<C: WorldContext> Chunk<C> {
    pub fn empty_with_world(world: &World<C>, pos: impl Into<ChunkLocation>) -> Self {
        Self {
            pos: pos.into(),
            terrain: RawChunkTerrain::default(),
            block_data: HashMap::new(),
            areas: HashMap::new(),
            slab_navs: HashMap::new(),
            vertical_space: HashMap::new(),
            slab_progress: RwLock::new(HashMap::new()),
            slab_notify: world.load_notifications(),
        }
    }

    /// Disconnected from any world's slab notifications
    #[cfg(test)]
    pub fn empty(pos: impl Into<ChunkLocation>) -> Self {
        Self {
            pos: pos.into(),
            terrain: RawChunkTerrain::default(),
            block_data: HashMap::new(),
            slab_navs: HashMap::new(),
            vertical_space: HashMap::new(),
            areas: HashMap::new(),
            slab_progress: RwLock::new(HashMap::new()),
            slab_notify: LoadNotifier::default(),
        }
    }

    #[inline]
    pub fn pos(&self) -> ChunkLocation {
        self.pos
    }

    pub fn id(&self) -> ChunkId {
        let ChunkLocation(x, y) = self.pos;
        (x as u64) << 32 | (y as u64)
    }

    pub fn area_info_for_block(&self, block: BlockPosition) -> Option<AreaInfo> {
        let b = self.get_block(block)?;
        let area = b.nav_area()?;
        self.areas
            .get(&ChunkArea {
                slab_idx: block.z().slab_index(),
                slab_area: SlabArea {
                    slice_idx: block.z().to_local(),
                    slice_area: area,
                },
            })
            .copied()
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
        self.get_block(block_pos).map(|b| {
            let area_index = b.area_index();
            WorldArea {
                chunk: self.pos,
                slab: block_pos.z().slab_index(),
                area: area_index,
            }
        })
    }

    pub(crate) fn areas(&self) -> impl Iterator<Item = &ChunkArea> {
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

    pub fn replace_all_slice_areas(
        &mut self,
        slab: SlabIndex,
        new_areas: impl Iterator<Item = SliceNavArea>,
    ) {
        // remove all for this slab
        self.areas.retain(|a, _| a.slab_idx != slab);

        let mut area_alloc = SliceAreaIndexAllocator::default();
        let tmp_add = new_areas
            .map(|a| {
                (
                    dbg!(ChunkArea {
                        slab_idx: slab,
                        slab_area: SlabArea {
                            slice_idx: a.slice,
                            slice_area: area_alloc.allocate(a.slice.slice()),
                        },
                    }),
                    AreaInfo {
                        height: a.height,
                        range: (a.from, a.to),
                    },
                )
            })
            .collect_vec();
        self.areas.extend(tmp_add.into_iter());
    }

    pub(crate) fn update_block_graphs(
        &mut self,
        slab_nav: impl Iterator<Item = (ChunkArea, BlockGraph)>,
    ) {
        // for (area, graph) in slab_nav {
        //     let (new_edges, new_nodes) = graph.len();
        //     self.areas.insert(area, graph);
        //     debug!("added {edges} edges and {nodes} nodes", edges = new_edges, nodes = new_nodes; "area" => ?area)
        // }
        unreachable!()
    }

    pub fn slice_range(
        &self,
        range: SliceRange,
    ) -> impl Iterator<Item = (GlobalSliceIndex, Slice<C>)> {
        range
            .as_range()
            .map(move |i| self.slice(i).map(|s| (GlobalSliceIndex::new(i), s)))
            .skip_while(|s| s.is_none())
            .while_some()
    }

    pub fn slice_or_dummy(&self, slice: GlobalSliceIndex) -> Slice<C> {
        #[allow(clippy::redundant_closure)]
        self.slice(slice).unwrap_or_else(|| Slice::dummy())
    }

    pub fn associated_block_data(&self, pos: BlockPosition) -> Option<&C::AssociatedBlockData> {
        self.block_data.get(&pos)
    }

    pub fn set_associated_block_data(
        &mut self,
        pos: BlockPosition,
        data: C::AssociatedBlockData,
    ) -> Option<C::AssociatedBlockData> {
        self.block_data.insert(pos, data)
    }

    pub fn remove_associated_block_data(
        &mut self,
        pos: BlockPosition,
    ) -> Option<C::AssociatedBlockData> {
        self.block_data.remove(&pos)
    }

    pub(crate) fn set_slab_nav_progress(&mut self, slab: SlabIndex, vs: SlabVerticalSpace) {
        self.vertical_space.insert(slab, vs);
    }

    pub(crate) fn mark_slab_requested(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::Requested);
        // no notification necessary, nothing waits for a slab to be requested
    }

    pub(crate) fn mark_slab_as_in_world(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::TerrainInWorld);
        self.slab_notify.notify(SlabLocation::new(slab, self.pos));
    }

    pub(crate) fn mark_slab_as_done_in_isolation(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::DoneInIsolation);
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
            SlabLoadingStatus::Done => {
                // is already loaded, only load again if it is a placeholder
                let slab = self.terrain.slab(slab).unwrap();
                slab.is_placeholder()
            }
            _ => {
                // is currently in progress, don't request again
                false
            }
        }
    }

    pub fn is_slab_loaded(&self, slab: SlabIndex) -> bool {
        let progress = self.slab_progress(slab);
        matches!(progress, SlabLoadingStatus::Done)
    }

    /// If true it has not been requested and is not loading
    pub fn slab_vertical_space_or_wait(&self, slab: SlabIndex) -> VerticalSpaceOrWait {
        use VerticalSpaceOrWait::*;
        let progress = self.slab_progress(slab);

        match progress {
            SlabLoadingStatus::Unloaded => Failure,
            SlabLoadingStatus::Requested => Wait,
            _ => Loaded(self.slab_vertical_space(slab).unwrap().clone()),
        }
    }

    pub fn has_slab(&self, slab: SlabIndex) -> bool {
        self.terrain.slab(slab).is_some()
    }

    pub fn slab_vertical_space(&self, slab: SlabIndex) -> Option<&SlabVerticalSpace> {
        self.vertical_space.get(&slab)
    }

    pub(crate) fn replace_slab_nav_graph(&mut self, slab: SlabIndex, graph: SlabNavGraph) {
        self.slab_navs.insert(slab, graph);
    }

    pub fn slab_nav_graph(&self, slab: SlabIndex) -> Option<&SlabNavGraph> {
        self.slab_navs.get(&slab)
    }
}

pub enum VerticalSpaceOrWait {
    Wait,
    Failure,
    Loaded(SlabVerticalSpace),
}

impl<C: WorldContext> BaseTerrain<C> for Chunk<C> {
    fn raw_terrain(&self) -> &RawChunkTerrain<C> {
        &self.terrain
    }

    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain<C> {
        &mut self.terrain
    }
}

#[cfg(test)]
mod tests {
    use unit::world::GlobalSliceIndex;

    use crate::chunk::terrain::BaseTerrain;
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
        c.blocks(&mut blocks);
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
