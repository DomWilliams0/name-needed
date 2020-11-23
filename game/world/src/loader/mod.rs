use parking_lot::Mutex;
use std::time::{Duration, Instant};

use crossbeam::channel::{Receiver, Sender};
use crossbeam::crossbeam_channel::{bounded, unbounded};

pub use batch::UpdateBatch;
use common::*;
pub use terrain_source::TerrainSource;
pub use terrain_source::{GeneratedTerrainSource, MemoryTerrainSource};
use unit::world::{ChunkLocation, SlabLocation};
pub use update::{GenericTerrainUpdate, SlabTerrainUpdate, TerrainUpdatesRes, WorldTerrainUpdate};
pub use worker_pool::{BlockingWorkerPool, ThreadedWorkerPool, WorkerPool};

use crate::chunk::slab::{Slab, SlabInternalNavigability, SlabTerrain};
use crate::chunk::{ChunkTerrain, RawChunkTerrain, SlabLoadingStatus, WhichChunk};
use crate::loader::batch::UpdateBatchUniqueId;
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::worker_pool::LoadTerrainResult;
use crate::world::WorldChangeEvent;
use crate::{OcclusionChunkUpdate, WorldRef};
use std::sync::Arc;

mod batch;
mod finalizer;
mod terrain_source;
mod update;
mod worker_pool;

pub struct WorldLoader<P: WorkerPool<D>, D> {
    // TODO use rwlock so preprocessing can start concurrently
    source: Arc<Mutex<dyn TerrainSource>>,
    pool: P,
    finalization_channel: Sender<LoadTerrainResult>,
    chunk_updates_rx: Option<Receiver<OcclusionChunkUpdate>>,
    world: WorldRef<D>,
    last_batch_size: usize,
    batch_ids: UpdateBatchUniqueId,
}

pub struct LoadedSlab {
    pub(crate) slab: SlabLocation,
    pub(crate) terrain: Slab,
    pub(crate) navigation: SlabInternalNavigability,
    pub(crate) batch: UpdateBatch,
}

#[derive(Debug, Error)]
pub enum BlockForAllError {
    #[error("A batch of chunks must be requested first")]
    NoBatch,

    #[error("Timed out")]
    TimedOut,

    #[error("Failed to load terrain: {0}")]
    Error(#[from] TerrainSourceError),
}

#[derive(Copy, Clone)]
// TODO slabs not chunks
pub enum ChunkRequest {
    New,
    UpdateExisting,
}

impl<P: WorkerPool<D>, D: 'static> WorldLoader<P, D> {
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
            last_batch_size: 0,
            batch_ids: UpdateBatchUniqueId::default(),
        }
    }

    pub fn world(&self) -> WorldRef<D> {
        self.world.clone()
    }

    /// Requests slabs as a single batch
    pub fn request_slabs(&mut self, slabs: impl ExactSizeIterator<Item = SlabLocation>) {
        let count = slabs.len();
        self.request_slabs_with_count(slabs, count)
    }

    // TODO more efficient version that takes chunk+multiple slabs
    pub fn request_slabs_with_count(
        &mut self,
        slabs: impl Iterator<Item = SlabLocation>,
        count: usize,
    ) {
        let mut batches = UpdateBatch::builder(&mut self.batch_ids, count);
        let mut world_mut = self.world.borrow_mut();
        let mut real_count = 0;

        // hold source lock as a barrier for this batch, until all slabs have been registered
        // with the chunk so they know if their neighbours are being loaded
        let _guard = self.source.lock();

        for slab in slabs {
            let chunk = world_mut.ensure_chunk(slab.chunk);
            chunk.mark_slab_requested(slab.slab);

            let source = self.source.clone();
            let batch = batches.next_batch();

            debug!(
                "submitting slab to pool as part of batch";
                slab, batch
            );

            // load raw terrain and do as much processing in isolation as possible on a worker thread
            let world = self.world();
            self.pool.submit(
                move || {
                    // wrapped in closure for common error handling case
                    let get_terrain = || -> Result<SlabTerrain, TerrainSourceError> {
                        // briefly hold the source lock to get a preprocess closure to run
                        let preprocess_work = {
                            let terrain_source = source.lock();
                            terrain_source.preprocess(slab)
                        };

                        // run preprocessing work concurrently
                        let preprocess_result = preprocess_work()?;

                        // take the source lock again to convert preprocessing output into raw terrain
                        // e.g. reading from a file cannot be done in parallel
                        let terrain = {
                            let mut terrain_source = source.lock();
                            terrain_source.load_slab(slab, preprocess_result)?
                        };

                        Ok(terrain)
                    };

                    let terrain = match get_terrain() {
                        Ok(terrain) => terrain,
                        Err(TerrainSourceError::OutOfBounds(slab)) => {
                            // soft error, we're at the world edge. treat as all air instead of
                            // crashing and burning
                            debug!("slab is out of bounds, swapping in an empty one"; slab);
                            // TODO CoW empty slab
                            SlabTerrain::empty(slab)
                        }
                        Err(err) => return Err(err),
                    };

                    // slab terrain is now fixed, copy the top and bottom slices into chunk so
                    // neighbouring slabs can access it
                    let world = world.borrow();
                    let chunk = world.find_chunk_with_pos(slab.chunk).unwrap();
                    chunk.mark_slab_in_progress(slab.slab, &terrain);

                    // wait for above+below slabs to be loaded if they're in progress, then
                    // concurrently process raw terrain in context of own chunk
                    let (above, below) = chunk.wait_for_neighbouring_slabs(slab.slab);
                    // TODO detect when slab is all air and avoid expensive processing
                    let (terrain, navigation) = terrain.into_real_slab(
                        above.as_ref().map(|s| s.into()), // gross
                        below.as_ref().map(|s| s.into()),
                    );

                    // submit slab for finalization
                    Ok(LoadedSlab {
                        slab,
                        terrain,
                        navigation,
                        batch,
                    })
                },
                self.finalization_channel.clone(),
            );

            real_count += 1;
        }

        // unblock workers
        drop(_guard);

        debug!("slab batch of size {size} submitted", size = count);

        assert_eq!(
            real_count, count,
            "expected batch of {} but actually got {}",
            count, real_count
        );

        self.last_batch_size = count;
    }

    fn update_chunks_with_len(
        &mut self,
        updates: impl Iterator<Item = (ChunkLocation, RawChunkTerrain)>,
        count: usize,
    ) {
        let mut batches = UpdateBatch::builder(&mut self.batch_ids, count);
        for (chunk, terrain) in updates {
            let batch = batches.next_batch();

            debug!(
                "submitting chunk update to pool";
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
                    todo!()
                    // Ok((chunk, terrain, batch))
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
        changes_out: &mut Vec<WorldChangeEvent>,
    ) {
        let world = self.world.clone();
        let world = world.borrow();

        let (slab_updates, chunk_count) = {
            // translate world -> slab updates
            // TODO reuse vec alloc
            let mut slab_updates = terrain_updates
                .flat_map(|world_update| world_update.into_slab_updates())
                .collect_vec();

            // sort then group by chunk and slab, so each slab is touched only once
            slab_updates.sort_unstable_by(|(chunk_a, slab_a, _), (chunk_b, slab_b, _)| {
                chunk_a.cmp(chunk_b).then(slab_a.cmp(slab_b))
            });

            // filter out unloaded chunks
            // TODO filter out unloaded slabs too
            // TODO this query a chunk repeatedly for every slab, only do this once per chunk preferably
            slab_updates.retain(|(chunk, _, _)| world.has_chunk(*chunk));

            // count chunk count for batch size. no allocations in dedup because vec is sorted
            let chunk_count = slab_updates
                .iter()
                .dedup_by(|(chunk_a, _, _), (chunk_b, _, _)| chunk_a == chunk_b)
                .count();

            (slab_updates, chunk_count)
        };

        let grouped_chunk_updates = slab_updates.into_iter().group_by(|(chunk, _, _)| *chunk);

        let chunk_updates = grouped_chunk_updates
            .into_iter()
            // TODO repeated filter check not needed?
            .filter(|(chunk, _)| world.has_chunk(*chunk))
            // apply updates to world in closure, don't submit them to the loader right now
            .map(|(chunk, updates)| {
                let updates = updates.map(|(_, slab, update)| (slab, update));
                let new_terrain = world.apply_terrain_updates(chunk, updates, changes_out);
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
    ) -> Option<Result<SlabLocation, TerrainSourceError>> {
        let end_time = Instant::now() + timeout;
        loop {
            let this_timeout = {
                let now = Instant::now();
                if now >= end_time {
                    break None; // finished
                }
                let left = end_time - now;
                left.min(Duration::from_secs(1))
            };

            if let ret @ Some(_) = self.pool.block_on_next_finalize(this_timeout) {
                break ret;
            }
        }
    }

    pub fn block_for_last_batch(&mut self, timeout: Duration) -> Result<(), BlockForAllError> {
        match self.last_batch_size {
            0 => Err(BlockForAllError::NoBatch),
            count => {
                let start_time = Instant::now();
                for i in 0..count {
                    let elapsed = start_time.elapsed();
                    let timeout = match timeout.checked_sub(elapsed) {
                        None => return Err(BlockForAllError::TimedOut),
                        Some(t) => t,
                    };

                    trace!("waiting for slab {index}/{total}", index = i + 1, total = count; "timeout" => ?timeout);
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

#[cfg(test)]
mod tests {

    use std::time::Duration;

    use unit::dim::CHUNK_SIZE;

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

        let mut loader = WorldLoader::<_, ()>::new(source, BlockingWorkerPool::default());
        // loader.request_chunks(once(ChunkLocation(0, 0)));
        todo!();

        let finalized = loader.block_on_next_finalization(Duration::from_secs(15));
        // assert_matches!(finalized, Some(Ok(ChunkLocation(0, 0))));
        todo!();

        assert_eq!(loader.world.borrow().all_chunks().count(), 1);
    }
}
