use common::*;

use unit::world::{
    BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SlabLocation,
    SliceBlock, WorldPosition,
};

use crate::block::BlockType;
use crate::chunk::slab::Slab;
use crate::chunk::slice::{Slice, SliceOwned};
use crate::chunk::terrain::RawChunkTerrain;
use crate::chunk::BaseTerrain;
use crate::navigation::{BlockGraph, ChunkArea, WorldArea};
use crate::world::LoadNotifier;
use crate::{SliceRange, World, WorldContext};
use parking_lot::RwLock;
use std::collections::HashMap;

pub type ChunkId = u64;

pub struct Chunk<C: WorldContext> {
    /// Unique for each chunk
    pos: ChunkLocation,

    terrain: RawChunkTerrain,

    /// Sparse associated data with each block
    block_data: HashMap<BlockPosition, C::AssociatedBlockData>,

    /// Navigation lookup
    areas: HashMap<ChunkArea, BlockGraph>,

    slab_progress: RwLock<HashMap<SlabIndex, SlabLoadingStatus>>,
    slab_notify: LoadNotifier,
}

#[derive(Clone, Debug)]
pub(crate) enum SlabLoadingStatus {
    /// Not available
    Unloaded,

    /// Has been requested
    Requested,

    /// Is in progress
    InProgress {
        /// Slab's top slice
        top: SliceOwned,

        /// Slab's bottom slice
        bottom: SliceOwned,
    },

    /// Finished and present in chunk.terrain
    Done,
}

enum SlabAvailability {
    Never,
    OnItsWay,
    TadaItsAvailable(SliceOwned),
}

impl<C: WorldContext> Chunk<C> {
    pub fn empty_with_world(world: &World<C>, pos: impl Into<ChunkLocation>) -> Self {
        Self {
            pos: pos.into(),
            terrain: RawChunkTerrain::default(),
            block_data: HashMap::new(),
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
            terrain: RawChunkTerrain::default(),
            block_data: HashMap::new(),
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

    pub fn get_block_type<B: Into<BlockPosition>>(&self, pos: B) -> Option<BlockType> {
        self.get_block(pos).map(|b| b.block_type())
    }

    pub(crate) fn area_for_block(&self, pos: WorldPosition) -> Option<WorldArea> {
        self.get_block(pos).map(|b| {
            let area_index = b.area_index();
            let block_pos: BlockPosition = pos.into();
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
        let n = self.areas.len();
        self.areas
            .retain(|area, _| !(area.slab >= min && area.slab <= max));
        let m = self.areas.len();
        debug!("removed {removed} nodes in slab range", removed=n-m; "lower"=>min, "upper"=>max);
    }

    pub(crate) fn block_graph_for_area(&self, area: WorldArea) -> Option<&BlockGraph> {
        self.areas.get(&area.into())
    }

    pub(crate) fn update_block_graphs(
        &mut self,
        slab_nav: impl Iterator<Item = (ChunkArea, BlockGraph)>,
    ) {
        for (area, graph) in slab_nav {
            let (new_edges, new_nodes) = graph.len();
            self.areas.insert(area, graph);
            debug!("added {edges} edges and {nodes} nodes", edges = new_edges, nodes = new_nodes; "area" => ?area)
        }
    }

    pub fn slice_range(
        &self,
        range: SliceRange,
    ) -> impl Iterator<Item = (GlobalSliceIndex, Slice)> {
        range
            .as_range()
            .map(move |i| self.slice(i).map(|s| (GlobalSliceIndex::new(i), s)))
            .skip_while(|s| s.is_none())
            .while_some()
    }

    pub fn slice_or_dummy(&self, slice: GlobalSliceIndex) -> Slice {
        #[allow(clippy::redundant_closure)]
        self.slice(slice).unwrap_or_else(|| Slice::dummy())
    }

    // TODO use an enum for the slice range rather than Options
    pub fn find_accessible_block(
        &self,
        pos: SliceBlock,
        start_from: Option<GlobalSliceIndex>,
        end_at: Option<GlobalSliceIndex>,
    ) -> Option<BlockPosition> {
        let start_from = start_from.unwrap_or_else(GlobalSliceIndex::top);
        let end_at = end_at.unwrap_or_else(GlobalSliceIndex::bottom);
        self.terrain
            .slices_from_top_offset()
            .skip_while(|(s, _)| *s > start_from)
            .take_while(|(s, _)| *s >= end_at)
            .find(|(_, slice)| slice[pos].walkable())
            .map(|(z, _)| pos.to_block_position(z))
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

    pub(crate) fn mark_slab_requested(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::Requested);
        // no notification necessary, nothing waits for a slab to be requested
    }

    pub(crate) fn mark_slab_in_progress(&self, slab: SlabIndex, terrain: &Slab) {
        let top = terrain.slice_owned(LocalSliceIndex::top());
        let bottom = terrain.slice_owned(LocalSliceIndex::bottom());

        self.update_slab_status(slab, SlabLoadingStatus::InProgress { top, bottom });
        self.slab_notify.notify(SlabLocation::new(slab, self.pos));
    }

    pub(crate) fn mark_slabs_complete(&self, slabs: impl Iterator<Item = SlabIndex> + Clone) {
        self.update_slabs_status(slabs.clone(), SlabLoadingStatus::Done);
        for slab in slabs {
            self.slab_notify.notify(SlabLocation::new(slab, self.pos));
        }
    }

    fn update_slab_status(&self, slab: SlabIndex, state: SlabLoadingStatus) {
        self.update_slabs_status(once(slab), state)
    }

    /// Does not notify
    fn update_slabs_status(
        &self,
        mut slabs: impl Iterator<Item = SlabIndex>,
        state: SlabLoadingStatus,
    ) {
        let mut map = self.slab_progress.write();

        let mut do_update = |slab, state| {
            trace!("updating slab progress"; SlabLocation::new(slab, self.pos), "state" => ?state);
            map.insert(slab, state);
        };

        let first = slabs.next();

        // clone for all except first
        for slab in slabs {
            do_update(slab, state.clone());
        }

        // use original for last
        if let Some(slab) = first {
            do_update(slab, state);
        }
    }

    /// None if should wait
    pub(crate) fn get_neighbouring_slabs(
        &self,
        slab: SlabIndex,
    ) -> Option<(Option<SliceOwned>, Option<SliceOwned>)> {
        // slice below is mandatory as it's used for navigation. if its in progress then we need to wait
        let below_availability = self.get_slab_slice_availability(slab - 1, true);

        let below = match below_availability {
            SlabAvailability::Never => {
                // make do without
                None
            }
            SlabAvailability::OnItsWay => {
                // wait required
                return None;
            }
            SlabAvailability::TadaItsAvailable(slice) => Some(slice),
        };

        // slice above is optional, only used to calculate occlusion which can be updated later
        let above = slab + 1;
        let above = self.get_slab_slice(self.slab_progress(above), above, false);

        Some((above, below))
    }

    fn slab_progress(&self, slab: SlabIndex) -> SlabLoadingStatus {
        let guard = self.slab_progress.read();
        guard
            .get(&slab)
            .unwrap_or(&SlabLoadingStatus::Unloaded)
            .clone()
    }

    fn get_slab_slice_availability(&self, slab: SlabIndex, top_slice: bool) -> SlabAvailability {
        let state = self.slab_progress(slab);
        match state {
            SlabLoadingStatus::InProgress { top, bottom } => {
                let slice = if top_slice { top } else { bottom };
                SlabAvailability::TadaItsAvailable(slice)
            }
            SlabLoadingStatus::Done => {
                let slice = if top_slice {
                    LocalSliceIndex::top()
                } else {
                    LocalSliceIndex::bottom()
                };
                let global_slice = slice.to_global(slab);
                let slice = self.terrain.slice(global_slice).unwrap_or_else(|| {
                    panic!(
                        "slab {:?} is apparently loaded but could not be found",
                        slab
                    )
                });
                SlabAvailability::TadaItsAvailable(slice.to_owned())
            }

            SlabLoadingStatus::Unloaded => SlabAvailability::Never,
            SlabLoadingStatus::Requested => SlabAvailability::OnItsWay,
        }
    }

    fn get_slab_slice(
        &self,
        state: SlabLoadingStatus,
        slab: SlabIndex,
        top_slice: bool,
    ) -> Option<SliceOwned> {
        match state {
            SlabLoadingStatus::InProgress { top, bottom } => {
                Some(if top_slice { top } else { bottom })
            }
            SlabLoadingStatus::Done => {
                let slice = if top_slice {
                    LocalSliceIndex::top()
                } else {
                    LocalSliceIndex::bottom()
                };
                let global_slice = slice.to_global(slab);
                let slice = self.terrain.slice(global_slice).unwrap_or_else(|| {
                    panic!(
                        "slab {:?} is apparently loaded but could not be found",
                        slab
                    )
                });
                Some(slice.to_owned())
            }
            _ => None,
        }
    }

    /// Returns true if slab has not been already requested/loaded or is a placeholder
    pub fn should_slab_be_loaded(&self, slab: SlabIndex) -> bool {
        match self.slab_progress(slab) {
            SlabLoadingStatus::Unloaded => {
                // has not been requested, pls load
                true
            }
            SlabLoadingStatus::Requested | SlabLoadingStatus::InProgress { .. } => {
                // is currently in progress, don't request again
                false
            }
            SlabLoadingStatus::Done => {
                // is already loaded, only load again if it is a placeholder
                let slab = self.terrain.slab(slab).unwrap();
                slab.is_placeholder()
            }
        }
    }

    pub fn has_slab(&self, slab: SlabIndex) -> bool {
        self.terrain.slab(slab).is_some()
    }
}

impl<C: WorldContext> BaseTerrain for Chunk<C> {
    fn raw_terrain(&self) -> &RawChunkTerrain {
        &self.terrain
    }

    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain {
        &mut self.terrain
    }
}

#[cfg(test)]
mod tests {
    use unit::world::GlobalSliceIndex;

    use crate::block::BlockType;
    use crate::chunk::terrain::BaseTerrain;
    use crate::chunk::{Chunk, ChunkBuilder};
    use crate::helpers::DummyWorldContext;
    use unit::world::CHUNK_SIZE;

    #[test]
    fn chunk_ops() {
        // check setting and getting blocks works
        let chunk = ChunkBuilder::new()
            .apply(|c| {
                // a bit on slice 0
                for i in 0..3 {
                    c.set_block((i, i, 0), BlockType::Dirt);
                }
            })
            .set_block((2, 3, 1), BlockType::Dirt)
            .into_inner();

        // slice 1 was filled
        assert_eq!(chunk.get_block_type((2, 3, 1)), Some(BlockType::Dirt));

        // collect slice
        let slice: Vec<BlockType> = chunk
            .slice(GlobalSliceIndex::new(0))
            .unwrap()
            .iter()
            .map(|b| b.block_type())
            .collect();
        assert_eq!(slice.len(), CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize()); // ensure exact length
        assert_eq!(slice.iter().filter(|b| **b != BlockType::Air).count(), 3); // ensure exact number of filled blocks

        // ensure each exact coord was filled
        assert_eq!(chunk.get_block_type((0, 0, 0)), Some(BlockType::Dirt));
        assert_eq!(chunk.get_block_type((1, 1, 0)), Some(BlockType::Dirt));
        assert_eq!(chunk.get_block_type((2, 2, 0)), Some(BlockType::Dirt));
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
            b.next().map(|(p, b)| (p, b.block_type())),
            Some(((0, 0, 0).into(), BlockType::Air))
        );
        assert_eq!(
            b.next().map(|(p, b)| (p, b.block_type())),
            Some(((1, 0, 0).into(), BlockType::Air))
        );
        assert_eq!(
            b.next().map(|(p, b)| (p, b.block_type())),
            Some(((2, 0, 0).into(), BlockType::Air))
        );
    }
}
