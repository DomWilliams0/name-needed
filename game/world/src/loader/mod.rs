use std::mem::MaybeUninit;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam::channel::{Receiver, Sender};
use crossbeam::crossbeam_channel::{bounded, unbounded};

pub use batch::UpdateBatch;
use common::derive_more::{Display, Error};
use common::*;
pub use terrain_source::TerrainSource;
pub use terrain_source::{GeneratedTerrainSource, MemoryTerrainSource};
use unit::world::ChunkPosition;
pub use update::{GenericTerrainUpdate, SlabTerrainUpdate, TerrainUpdatesRes, WorldTerrainUpdate};
pub use worker_pool::{BlockingWorkerPool, ThreadedWorkerPool, WorkerPool};

use crate::chunk::{BaseTerrain, Chunk, ChunkTerrain, RawChunkTerrain, WhichChunk};
use crate::loader::batch::{UpdateBatchUniqueId, UpdateBatcher};
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::worker_pool::LoadTerrainResult;
use crate::navigation::AreaNavEdge;
use crate::neighbour::NeighbourOffset;
use crate::{InnerWorldRef, OcclusionChunkUpdate, WorldRef};
use std::cell::{Cell, RefCell};
use std::ops::DerefMut;

mod batch;
mod terrain_source;
mod update;
mod worker_pool;

pub struct WorldLoader<P: WorkerPool> {
    source: Arc<Mutex<dyn TerrainSource>>,
    pool: P,
    finalization_channel: Sender<LoadTerrainResult>,
    chunk_updates_rx: Option<Receiver<OcclusionChunkUpdate>>,
    world: WorldRef,
    all_count: Option<usize>,
    batch_ids: UpdateBatchUniqueId,
}

struct ChunkFinalizer {
    world: WorldRef,
    updates: Sender<OcclusionChunkUpdate>,
    batcher: UpdateBatcher<FinalizeBatchItem>,
}

struct FinalizeBatchItem {
    chunk: ChunkPosition,
    terrain: RefCell<MaybeUninit<ChunkTerrain>>,
    consumed: Cell<bool>,
}

#[derive(Debug, Display, Error)]
pub enum BlockForAllError {
    /// Terrain source needs to have had `request_all` called to know how many to wait for
    #[display(fmt = "`request_all` must be called first")]
    Unsupported,

    #[display("Timed out")]
    TimedOut,

    #[display(fmt = "Failed to load terrain")]
    Error(TerrainSourceError),
}

#[derive(Copy, Clone)]
pub enum ChunkRequest {
    New,
    UpdateExisting,
}

impl<P: WorkerPool> WorldLoader<P> {
    pub fn new<S: TerrainSource + 'static>(source: S, mut pool: P) -> Self {
        let (finalize_tx, finalize_rx) = bounded(16);
        let (chunk_updates_tx, chunk_updates_rx) = unbounded();

        let world = WorldRef::default();
        pool.start_finalizer(world.clone(), finalize_rx, chunk_updates_tx);

        Self {
            source: Arc::new(Mutex::new(source)),
            pool,
            finalization_channel: finalize_tx,
            chunk_updates_rx: Some(chunk_updates_rx),
            world,
            all_count: None,
            batch_ids: UpdateBatchUniqueId::default(),
        }
    }

    pub fn world(&self) -> WorldRef {
        self.world.clone()
    }

    pub fn request_all_chunks(&mut self) {
        let chunks = self.source.lock().unwrap().all_chunks();
        let count = chunks.len();
        self.request_chunks(chunks.into_iter());

        self.all_count = Some(count);
    }

    /// Requests chunks as a single batch
    pub fn request_chunks(&mut self, chunks: impl ExactSizeIterator<Item = ChunkPosition>) {
        // TODO cache full finalized chunks

        let mut batches = UpdateBatch::builder(&mut self.batch_ids, chunks.len());

        for chunk in chunks {
            let source = self.source.clone();
            let batch = batches.next_batch();

            debug!(
                "submitting chunk {:?} request to pool in batch {:?}",
                chunk, batch
            );

            // load raw terrain and do as much processing in isolation as possible on a worker thread
            self.pool.submit(
                move || {
                    // briefly hold the source lock to get a preprocess closure to run
                    let preprocess_work = {
                        let terrain_source = source.lock().unwrap();

                        // fail fast if bad chunk position
                        if !terrain_source.is_in_bounds(chunk) {
                            return Err(TerrainSourceError::OutOfBounds);
                        }

                        terrain_source.preprocess(chunk)
                    };

                    // run preprocessing work concurrently
                    let preprocess_result = preprocess_work()?;

                    // take the source lock again to convert preprocessing output into raw terrain.
                    // e.g. reading from a file cannot be done in parallel
                    let terrain = {
                        let mut terrain_source = source.lock().unwrap();
                        terrain_source.load_chunk(chunk, preprocess_result)?
                    };

                    // concurrently process raw terrain into chunk terrain
                    let terrain = ChunkTerrain::from_raw_terrain(terrain, chunk, ChunkRequest::New);
                    Ok((chunk, terrain, batch))
                },
                self.finalization_channel.clone(),
            );
        }
    }

    fn update_chunks_with_len(
        &mut self,
        updates: impl Iterator<Item = (ChunkPosition, RawChunkTerrain)>,
        count: usize,
    ) {
        let mut batches = UpdateBatch::builder(&mut self.batch_ids, count);
        for (chunk, terrain) in updates {
            let batch = batches.next_batch();

            debug!(
                "submitting chunk {:?} update to pool in batch {:?}",
                chunk, batch
            );
            self.pool.submit(
                move || {
                    // concurrently process raw terrain into chunk terrain
                    let terrain = ChunkTerrain::from_raw_terrain(
                        terrain,
                        chunk,
                        ChunkRequest::UpdateExisting,
                    );
                    Ok((chunk, terrain, batch))
                },
                self.finalization_channel.clone(),
            );
        }

        if let Err((n, m)) = batches.is_complete() {
            panic!(
                "incorrect batch size, only produced {}/{} updates in batch",
                n, m
            );
        }
    }

    pub fn apply_terrain_updates(
        &mut self,
        terrain_updates: impl Iterator<Item = WorldTerrainUpdate>,
    ) {
        let world_ref = self.world.clone();
        let world = world_ref.borrow();

        let (slab_updates, chunk_count) = {
            // translate world -> slab updates
            let mut slab_updates = terrain_updates
                .flat_map(|world_update| world_update.into_slab_updates())
                .collect_vec();

            // sort then group by chunk and slab, so each slab is touched only once
            slab_updates.sort_unstable_by(|(chunk_a, slab_a, _), (chunk_b, slab_b, _)| {
                chunk_a.cmp(chunk_b).then(slab_a.cmp(slab_b))
            });

            // filter out unloaded chunks
            slab_updates.retain(|(chunk, _, _)| world.has_chunk(*chunk));

            // count chunk count for batch size
            let chunk_count = slab_updates
                .iter()
                .dedup_by(|(chunk_a, _, _), (chunk_b, _, _)| chunk_a == chunk_b)
                .count();

            (slab_updates, chunk_count)
        };

        let grouped_chunk_updates = slab_updates.into_iter().group_by(|(chunk, _, _)| *chunk);

        let chunk_updates = grouped_chunk_updates
            .into_iter()
            .filter(|(chunk, _)| world.has_chunk(*chunk))
            // apply updates to world but don't submit them to the loader yet
            .map(|(chunk, updates)| {
                let updates = updates.map(|(_, slab, update)| (slab, update));
                let new_terrain = world.apply_terrain_updates(chunk, updates);
                (chunk, new_terrain)
            });

        // submit all new terrain as a single batch
        if chunk_count > 0 {
            self.update_chunks_with_len(chunk_updates, chunk_count);
        }
    }

    pub fn block_on_next_finalization(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<ChunkPosition, TerrainSourceError>> {
        self.pool.block_on_next_finalize(timeout)
    }

    pub fn block_for_all(&mut self, timeout: Duration) -> Result<(), BlockForAllError> {
        match self.all_count {
            None => Err(BlockForAllError::Unsupported),
            Some(count) => {
                let start_time = Instant::now();
                for i in 0..count {
                    let elapsed = start_time.elapsed();
                    let timeout = match timeout.checked_sub(elapsed) {
                        None => return Err(BlockForAllError::TimedOut),
                        Some(t) => t,
                    };

                    trace!("waiting for chunk {}/{} for {:?}", (i + 1), count, timeout);
                    match self.block_on_next_finalization(timeout) {
                        None => return Err(BlockForAllError::TimedOut),
                        Some(Err(e)) => return Err(BlockForAllError::Error(e)),
                        Some(Ok(_)) => continue,
                    }
                }

                Ok(())
            }
        }
    }

    /// Takes `Receiver` out of loader
    pub fn chunk_updates_rx(&mut self) -> Option<Receiver<OcclusionChunkUpdate>> {
        self.chunk_updates_rx.take()
    }

    /// Borrows a clone of `Receiver`, leaving it in the loader
    pub fn chunk_updates_rx_clone(&mut self) -> Option<Receiver<OcclusionChunkUpdate>> {
        self.chunk_updates_rx.clone()
    }
}

impl ChunkRequest {
    pub fn is_new(self) -> bool {
        matches!(self, ChunkRequest::New)
    }
}

impl ChunkFinalizer {
    fn new(world: WorldRef, updates: Sender<OcclusionChunkUpdate>) -> Self {
        Self {
            world,
            updates,
            batcher: UpdateBatcher::default(),
        }
    }

    fn finalize(&mut self, (chunk, terrain, batch): (ChunkPosition, ChunkTerrain, UpdateBatch)) {
        // world lock is taken and released often to prevent holding up the main thread

        debug!(
            "submitting completed chunk {:?} for finalization in batch {:?}",
            chunk, batch
        );
        self.batcher
            .submit(batch, FinalizeBatchItem::initialized(chunk, terrain));

        // finalize completed batches only, which might not include this update.
        for (batch_id, batch_size) in self.batcher.complete_batches() {
            debug!("finalizing {} items in batch id={}", batch_size, batch_id);

            // we know that all dependent chunks (read: chunks in the same batch) are present now
            let batch = self.batcher.pop_batch(batch_id);
            trace!("batch: {:#?}", batch);
            debug_assert_eq!(batch.len(), batch_size);

            for idx in 0..batch_size {
                // pop this chunk from the dependent list
                let (chunk, terrain) = unsafe { batch.get_unchecked(idx) }.consume();

                // finalize this chunk
                debug!("finalizing chunk {:?}", chunk);
                self.do_finalize(chunk, terrain, &batch);
            }
        }
    }

    fn do_finalize<'func, 'dependents: 'func>(
        &'func mut self,
        chunk: ChunkPosition,
        mut terrain: ChunkTerrain,
        dependents: &'dependents [FinalizeBatchItem],
    ) {
        let lookup_neighbour =
            |chunk_pos, world: &InnerWorldRef| -> Option<&'dependents RawChunkTerrain> {
                // check finalize queue in case this chunk is dependent on the one being processed currently
                let dependent = dependents.iter().find_map(|item| item.get(chunk_pos));

                dependent.or_else(move || {
                    // check world as normal if its not a dependent
                    world
                        .find_chunk_with_pos(chunk_pos)
                        .map(|c| c.raw_terrain())
                        .map(|c| {
                            // TODO sort out the lifetimes instead of cheating and using transmute
                            // I can't get these lifetimes to agree, perhaps you the future reader can help
                            //  - dependents outlives this function
                            //  - this function returns terrain either from dependents or from the world
                            //  - the returned reference is only used for a single iteration of a loop
                            unsafe { std::mem::transmute(c) }
                        })
                })
            };

        for (direction, offset) in NeighbourOffset::offsets() {
            let world = self.world.borrow();

            let neighbour_offset = chunk + offset;
            let neighbour_terrain = match lookup_neighbour(neighbour_offset, &world) {
                Some(terrain) => terrain,
                // chunk is not loaded
                None => continue,
            };

            // TODO reuse/pool bufs, and initialize with proper expected size
            let mut this_terrain_updates = Vec::with_capacity(1200);
            let mut other_terrain_updates = Vec::with_capacity(1200);

            terrain.raw_terrain().cross_chunk_pairs_foreach(
                neighbour_terrain,
                direction,
                |which, block_pos, opacity| {
                    // TODO is it worth attempting to filter out updates that have no effect during the loop, or keep filtering them during consumption instead
                    //
                    match which {
                        WhichChunk::ThisChunk => {
                            // update opacity now for this chunk being loaded
                            this_terrain_updates.push((block_pos, opacity));
                        }
                        WhichChunk::OtherChunk => {
                            // queue opacity changes for the other chunk
                            other_terrain_updates.push((block_pos, opacity));
                        }
                    }
                },
            );

            // apply opacity changes to this chunk now
            {
                let applied = terrain
                    .raw_terrain_mut()
                    .apply_occlusion_updates(&this_terrain_updates);

                if applied > 0 {
                    debug!(
                        "applied {:?}/{:?} occlusion updates to chunk {:?}",
                        applied,
                        this_terrain_updates.len(),
                        chunk
                    );
                }

                this_terrain_updates.clear();
            }

            // queue changes to existing chunks in world
            if !other_terrain_updates.is_empty() {
                debug!(
                    "queueing {:?} occlusion updates to apply to chunk {:?} next tick",
                    other_terrain_updates.len(),
                    neighbour_offset
                );
                self.updates
                    .send(OcclusionChunkUpdate(
                        neighbour_offset,
                        other_terrain_updates,
                    ))
                    .unwrap();
            }
        }

        // navigation
        let mut area_edges = Vec::new(); // TODO reuse buf
        for (direction, offset) in NeighbourOffset::aligned() {
            let world = self.world.borrow();

            let neighbour_offset = chunk + offset;
            let neighbour_terrain = match lookup_neighbour(neighbour_offset, &world) {
                Some(terrain) => terrain,
                // chunk is not loaded
                None => continue,
            };

            let mut links = Vec::new(); // TODO reuse buf
            let mut ports = Vec::new(); // TODO reuse buf
            terrain.raw_terrain().cross_chunk_pairs_nav_foreach(
                    neighbour_terrain,
                    direction,
                    |src_area, dst_area, edge_cost, i, z| {
                        trace!("{:?} link from chunk {:?} area {:?} ----> chunk {:?} area {:?} ============= direction {:?} {}",
                               edge_cost, chunk, src_area, neighbour_offset, dst_area, direction, i);

                        let src_area = src_area.into_world_area(chunk);
                        let dst_area = dst_area.into_world_area(neighbour_offset);

                        links.push((src_area, dst_area, edge_cost, i, z));
                    },
                );

            links.sort_unstable_by_key(|(_, _, _, i, _)| *i);

            for ((src_area, dst_area), group) in links
                .iter()
                .group_by(|(src, dst, _, _, _)| (src, dst))
                .into_iter()
            {
                let direction = NeighbourOffset::between_aligned(src_area.chunk, dst_area.chunk);

                AreaNavEdge::discover_ports_between(
                    direction,
                    group.map(|(_, _, cost, idx, z)| (*cost, *idx, *z)),
                    &mut ports,
                );
                for edge in ports.drain(..) {
                    area_edges.push((*src_area, *dst_area, edge));
                }
            }
        }

        let chunk = Chunk::with_completed_terrain(chunk, terrain);
        {
            // finally take WorldRef write lock and post new chunk
            let mut world = self.world.borrow_mut();
            debug!("adding completed chunk {:?} to world", chunk.pos());
            world.add_loaded_chunk(chunk, &area_edges);
        }
    }
}

impl FinalizeBatchItem {
    fn initialized(chunk: ChunkPosition, terrain: ChunkTerrain) -> Self {
        Self {
            chunk,
            terrain: RefCell::new(MaybeUninit::new(terrain)),
            consumed: Cell::new(false),
        }
    }

    fn consume(&self) -> (ChunkPosition, ChunkTerrain) {
        let was_consumed = self.consumed.replace(true);
        assert!(!was_consumed, "chunk has already been consumed");

        // steal terrain
        let terrain = unsafe {
            let mut t = self.terrain.borrow_mut();
            std::mem::replace(t.deref_mut(), MaybeUninit::uninit()).assume_init()
        };

        (self.chunk, terrain)
    }

    fn get(&self, chunk_pos: ChunkPosition) -> Option<&RawChunkTerrain> {
        if self.consumed.get() || self.chunk != chunk_pos {
            None
        } else {
            let terrain_ref = self.terrain.borrow();
            let chunk_terrain = unsafe { &*terrain_ref.as_ptr() };
            Some(chunk_terrain.raw_terrain())
        }
    }
}

impl Debug for FinalizeBatchItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "FinalizeBatchItem(chunk={:?}, consumed={:?})",
            self.chunk,
            self.consumed.get()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;
    use std::time::Duration;

    use matches::assert_matches;

    use unit::dim::CHUNK_SIZE;
    use unit::world::ChunkPosition;

    use crate::block::BlockType;
    use crate::chunk::ChunkBuilder;
    use crate::loader::terrain_source::MemoryTerrainSource;
    use crate::loader::worker_pool::BlockingWorkerPool;
    use crate::loader::WorldLoader;

    #[test]
    fn thread_flow() {
        let a = ChunkBuilder::new()
            .set_block((0, 4, 60), BlockType::Stone)
            .into_inner();

        let b = ChunkBuilder::new()
            .set_block((CHUNK_SIZE.as_i32() - 1, 4, 60), BlockType::Grass)
            .into_inner();

        let source =
            MemoryTerrainSource::from_chunks(vec![((0, 0), a), ((-1, 0), b)].into_iter()).unwrap();

        let mut loader = WorldLoader::new(source, BlockingWorkerPool::default());
        loader.request_chunks(once(ChunkPosition(0, 0)));

        let finalized = loader.block_on_next_finalization(Duration::from_secs(15));
        assert_matches!(finalized, Some(Ok(ChunkPosition(0, 0))));

        assert_eq!(loader.world.borrow().all_chunks().count(), 1);
    }
}
