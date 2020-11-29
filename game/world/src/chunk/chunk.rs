use common::*;

use unit::world::{
    BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SliceBlock,
    WorldPosition,
};

use crate::block::BlockType;
use crate::chunk::slab::Slab;
use crate::chunk::slice::{Slice, SliceOwned};
use crate::chunk::terrain::RawChunkTerrain;
use crate::chunk::BaseTerrain;
use crate::navigation::{BlockGraph, ChunkArea, WorldArea};
use crate::SliceRange;
use parking_lot::{Condvar, Mutex, RwLock};
use std::collections::HashMap;

pub type ChunkId = u64;

pub struct Chunk<D> {
    /// Unique for each chunk
    pos: ChunkLocation,

    terrain: RawChunkTerrain,

    /// Sparse associated data with each block
    block_data: HashMap<BlockPosition, D>,

    /// Navigation lookup
    areas: HashMap<ChunkArea, BlockGraph>,

    slab_progress: RwLock<HashMap<SlabIndex, SlabLoadingStatus>>,
    slab_wait: Mutex<()>,
    slab_wait_cvar: Condvar,
}

#[derive(Clone, Debug)]
pub(crate) enum SlabLoadingStatus {
    /// Not available
    Unloaded,

    /// Has been requested
    Requested,

    /// Is in progress
    // TODO box these? this variant is 6K
    InProgress {
        /// Slab's top slice
        top: SliceOwned,

        /// Slab's bottom slice
        bottom: SliceOwned,
    },

    /// Finished and present in chunk.terrain
    Done,
}

impl<D> Chunk<D> {
    pub fn empty(pos: impl Into<ChunkLocation>) -> Self {
        Self {
            pos: pos.into(),
            terrain: RawChunkTerrain::default(),
            block_data: HashMap::new(),
            areas: HashMap::new(),
            slab_progress: RwLock::new(HashMap::new()),
            slab_wait: Mutex::new(()),
            slab_wait_cvar: Condvar::new(),
        }
    }

    pub const fn pos(&self) -> ChunkLocation {
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

    pub(crate) fn block_graph_for_area(&self, area: WorldArea) -> Option<&BlockGraph> {
        self.areas.get(&area.into())
    }

    pub(crate) fn update_block_graphs(
        &mut self,
        slab_nav: impl Iterator<Item = (ChunkArea, BlockGraph)>,
    ) {
        for (area, graph) in slab_nav {
            let (new_edges, new_nodes) = graph.len();
            match self.areas.insert(area, graph) {
                None => {
                    debug!("added {edges} edges and {nodes} nodes", edges = new_edges, nodes = new_nodes; "area" => ?area)
                }
                Some(prev) => {
                    let (prev_edges, prev_nodes) = prev.len();
                    debug!(
                        "replaced {prev_edges} edges and {prev_nodes} nodes with {new_edges} \
                        edges and {new_nodes} nodes",
                        prev_edges = prev_edges,
                        prev_nodes = prev_nodes,
                        new_edges = new_edges,
                        new_nodes = new_nodes
                    );
                }
            }
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

    pub fn associated_block_data(&self, pos: BlockPosition) -> Option<&D> {
        self.block_data.get(&pos)
    }

    pub fn set_associated_block_data(&mut self, pos: BlockPosition, data: D) -> Option<D> {
        self.block_data.insert(pos, data)
    }

    pub fn remove_associated_block_data(&mut self, pos: BlockPosition) -> Option<D> {
        self.block_data.remove(&pos)
    }

    pub(crate) fn mark_slab_requested(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::Requested);
    }

    pub(crate) fn mark_slab_in_progress(&self, slab: SlabIndex, terrain: &Slab) {
        let top = terrain.slice_owned(LocalSliceIndex::top());
        let bottom = terrain.slice_owned(LocalSliceIndex::bottom());

        self.update_slab_status(slab, SlabLoadingStatus::InProgress { top, bottom });
    }

    // TODO mark complete after finalization
    pub(crate) fn mark_slab_complete(&self, slab: SlabIndex) {
        self.update_slab_status(slab, SlabLoadingStatus::Done);
    }

    fn update_slab_status(&self, slab: SlabIndex, state: SlabLoadingStatus) {
        trace!("updating slab progress"; slab, "state" => ?state);

        let mut map = self.slab_progress.write();
        map.insert(slab, state);

        self.slab_wait_cvar.notify_all();
    }

    /// (slice above, slice below)
    pub(crate) fn wait_for_neighbouring_slabs(
        &self,
        slab: SlabIndex,
    ) -> (Option<SliceOwned>, Option<SliceOwned>) {
        // slice below is mandatory as it's used for navigation. wait for it if it's in progress
        let slice_below = self.wait_for_slab(slab - 1, true);

        // slice above is optional, only used to calculate occlusion which can be updated later
        let slice_above = {
            let above = slab + 1;
            let progress = self.slab_progress(above);
            self.get_slab_slice(progress, above, false)
        };

        (slice_above, slice_below)
    }

    fn slab_progress(&self, slab: SlabIndex) -> SlabLoadingStatus {
        let guard = self.slab_progress.read();
        guard
            .get(&slab)
            .unwrap_or(&SlabLoadingStatus::Unloaded)
            .clone()
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

    fn wait_for_slab(&self, slab: SlabIndex, top_slice: bool) -> Option<SliceOwned> {
        let mut ret = None;

        let mut should_keep_waiting = || {
            let state = self.slab_progress(slab);

            match state {
                SlabLoadingStatus::Requested => {
                    // keep waiting
                    trace!("waiting for other slab"; "index" => ?slab);
                    true
                }
                SlabLoadingStatus::Unloaded => {
                    // nothing to wait for
                    debug!("slab of interest is unloaded"; slab);
                    false
                }

                _ => {
                    // it's available
                    debug!("slab of interest is available"; slab, "state" => ?state);
                    ret = self.get_slab_slice(state, slab, top_slice);
                    false
                }
            }
        };

        let mut guard = self.slab_wait.lock();

        while should_keep_waiting() {
            self.slab_wait_cvar.wait(&mut guard);
        }

        ret
    }
}

impl<D> BaseTerrain for Chunk<D> {
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
    use unit::dim::CHUNK_SIZE;

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
        let id1 = Chunk::<()>::empty((0, 0)).id();
        let id2 = Chunk::<()>::empty((0, 1)).id();
        let id3 = Chunk::<()>::empty((1, 0)).id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn blocks() {
        // check individual block collection is ordered as intended
        let c = Chunk::<()>::empty((0, 0));
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
