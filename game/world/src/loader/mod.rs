use std::time::{Duration, Instant};

use futures::channel::mpsc as async_channel;

pub use batch::UpdateBatch;
use common::*;
pub use terrain_source::TerrainSource;
pub use terrain_source::{GeneratedTerrainSource, MemoryTerrainSource};
use unit::world::{SlabIndex, SlabLocation};
pub use update::{GenericTerrainUpdate, SlabTerrainUpdate, TerrainUpdatesRes, WorldTerrainUpdate};
pub use worker_pool::{AsyncWorkerPool, WorkerPool};

use crate::chunk::slab::{Slab, SlabInternalNavigability};

use crate::loader::batch::UpdateBatchUniqueId;
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::worker_pool::LoadTerrainResult;
use crate::world::WorldChangeEvent;
use crate::{OcclusionChunkUpdate, WorldRef};

use common::parking_lot::RwLock;
use std::iter::repeat;
use std::sync::Arc;

mod batch;
mod finalizer;
mod terrain_source;
mod update;
mod worker_pool;

pub struct WorldLoader<P: WorkerPool<D>, D> {
    source: Arc<RwLock<dyn TerrainSource>>,
    pool: P,
    finalization_channel: async_channel::Sender<LoadTerrainResult>,
    chunk_updates_rx: async_channel::UnboundedReceiver<OcclusionChunkUpdate>,
    world: WorldRef<D>,
    last_batch_size: usize,
    batch_ids: UpdateBatchUniqueId,
}

pub struct LoadedSlab {
    pub(crate) slab: SlabLocation,
    /// If None the terrain has already been updated
    pub(crate) terrain: Option<Slab>,
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
        let (finalize_tx, finalize_rx) = async_channel::channel(16);
        let (chunk_updates_tx, chunk_updates_rx) = async_channel::unbounded();

        let world = WorldRef::default();
        pool.start_finalizer(world.clone(), finalize_rx, chunk_updates_tx);

        Self {
            source: Arc::new(RwLock::new(source)),
            pool,
            finalization_channel: finalize_tx,
            chunk_updates_rx,
            world,
            last_batch_size: 0,
            batch_ids: UpdateBatchUniqueId::default(),
        }
    }

    pub fn world(&self) -> WorldRef<D> {
        self.world.clone()
    }

    /// Requests slabs as a single batch. Must be sorted as per [self.request_slabs_with_count]
    pub fn request_slabs(&mut self, slabs: impl ExactSizeIterator<Item = SlabLocation> + Clone) {
        let count = slabs.len();
        self.request_slabs_with_count(slabs, count)
    }

    // TODO add more efficient version that takes chunk+multiple slabs
    /// Must be sorted by chunk then by ascending slab (debug asserted)
    pub fn request_slabs_with_count(
        &mut self,
        slabs: impl Iterator<Item = SlabLocation> + Clone,
        count: usize,
    ) {
        let mut world_mut = self.world.borrow_mut();

        // check order of slabs is as expected
        if cfg!(debug_assertions) {
            let sorted = slabs
                .clone()
                .sorted_by(|a, b| a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab)));

            assert_equal(slabs.clone(), sorted);
        }

        let mut extra_slabs = SmallVec::<[SlabLocation; 16]>::new();

        // first iterate slabs to register them all with their chunk, so they know if their
        // neighbours have been requested/are being loaded too
        for (chunk, slabs) in slabs.clone().group_by(|slab| slab.chunk).into_iter() {
            log_scope!(o!(chunk));

            let chunk = world_mut.ensure_chunk(chunk);

            // track the highest slab
            let mut highest = SlabIndex::MIN;

            for slab in slabs {
                chunk.mark_slab_requested(slab.slab);

                debug_assert!(slab.slab > highest, "slabs should be in ascending order");
                highest = slab.slab;
            }

            // should have seen some slabs
            assert_ne!(highest, SlabIndex::MIN);

            // request the slab above the highest as all-air, so navigation discovery works
            // properly
            let empty = highest + 1;
            extra_slabs.push(SlabLocation::new(empty, chunk.pos()));
            chunk.mark_slab_requested(empty);
        }

        let count = count + extra_slabs.len();
        let mut batches = UpdateBatch::builder(&mut self.batch_ids, count);
        let mut real_count = 0;

        #[derive(Copy, Clone)]
        enum SlabRequest {
            Real,
            AirOnly,
        }

        let all_slabs = {
            let real_slabs = slabs.zip(repeat(SlabRequest::Real));
            let air_slabs = extra_slabs.into_iter().zip(repeat(SlabRequest::AirOnly));
            real_slabs.chain(air_slabs)
        };

        for (slab, request) in all_slabs {
            log_scope!(o!(slab));

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
                    let get_terrain = || -> Result<Slab, TerrainSourceError> {
                        if matches!(request, SlabRequest::AirOnly) {
                            // bit of a hack to force an all air slab
                            return Err(TerrainSourceError::OutOfBounds(slab));
                        }

                        // briefly hold the source lock to get a preprocess closure to run.
                        // only read lock so all workers can do this concurrently
                        let preprocess_work = {
                            let terrain_source = source.read();
                            terrain_source.preprocess(slab)
                        };

                        // run preprocessing work concurrently
                        let preprocess_result = preprocess_work()?;

                        // take the source write lock to convert preprocessing output into raw terrain
                        // e.g. reading from a file cannot be done in parallel
                        let terrain = {
                            let mut terrain_source = source.write();
                            terrain_source.load_slab(slab, preprocess_result)?
                        };

                        Ok(terrain)
                    };

                    let mut terrain = match get_terrain() {
                        Ok(terrain) => terrain,
                        Err(TerrainSourceError::OutOfBounds(slab)) => {
                            match request {
                                SlabRequest::AirOnly => {
                                    debug!("adding air only slab to the top of the chunk"; slab)
                                }
                                SlabRequest::Real => {
                                    // soft error, we're at the world edge. treat as all air instead of
                                    // crashing and burning
                                    debug!("slab is out of bounds, swapping in an empty one"; slab);
                                }
                            }

                            // TODO shared instance of CoW for empty slab
                            Slab::empty()
                        }
                        Err(err) => return Err(err),
                    };

                    // slab terrain is now fixed, process it concurrently on worker thread
                    let world = world.borrow();
                    let navigation = world
                        .process_given_slab_terrain(slab, &mut terrain)
                        .expect("chunk should be present");

                    // submit slab for finalization
                    Ok(LoadedSlab {
                        slab,
                        terrain: Some(terrain),
                        navigation,
                        batch,
                    })
                },
                self.finalization_channel.clone(),
            );

            real_count += 1;
        }

        debug!("slab batch of size {size} submitted", size = count);

        assert_eq!(
            real_count, count,
            "expected batch of {} but actually got {}",
            count, real_count
        );

        self.last_batch_size = count;
    }

    /// Note changes are made immediately to the terrain but are not immediate to the player,
    /// because navigation/occlusion/finalization is queued to the loader thread pool.
    // noinspection RsUnresolvedReference - itertools' GroupBy confuses CLion
    pub fn apply_terrain_updates(
        &mut self,
        terrain_updates: impl Iterator<Item = WorldTerrainUpdate>,
        changes_out: &mut Vec<WorldChangeEvent>,
    ) {
        let world_ref = self.world.clone();

        let (slab_updates, upper_slab_limit) = {
            // translate world -> slab updates
            // TODO reuse vec alloc
            let mut slab_updates = terrain_updates
                .flat_map(|world_update| world_update.into_slab_updates())
                .collect_vec();

            // sort then group by chunk and slab, so each slab is touched only once
            slab_updates.sort_unstable_by(|(a, _), (b, _)| {
                a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab))
            });

            // count slabs for vec allocation, upper limit because some might be filtered out.
            // no allocations in dedup because vec is sorted
            let upper_slab_limit = slab_updates
                .iter()
                .dedup_by(|(a, _), (b, _)| a == b)
                .count();

            (slab_updates, upper_slab_limit)
        };

        if upper_slab_limit == 0 {
            // nothing to do
            return;
        }

        // group per slab to each slab is fetched and modified only once
        let grouped_updates = slab_updates.into_iter().group_by(|(slab, _)| *slab);
        let grouped_updates = grouped_updates
            .into_iter()
            .map(|(slab, updates)| (slab, updates.map(|(_, update)| update)));

        // modify slabs in place - even though the changes won't be fully visible in the game yet (in terms of
        // navigation or rendering), world queries in the next game tick will be current with the
        // changes applied now.
        let mut slab_locs = Vec::with_capacity(upper_slab_limit);
        let mut world = world_ref.borrow_mut();
        world.apply_terrain_updates_in_place(
            grouped_updates.into_iter(),
            changes_out,
            |slab_loc| slab_locs.push(slab_loc),
        );

        let real_slab_count = slab_locs.len();
        debug!(
            "applied terrain updates to {count} slabs",
            count = real_slab_count
        );
        debug_assert_eq!(upper_slab_limit, slab_locs.capacity());

        // submit slabs for finalization
        let mut batches = UpdateBatch::builder(&mut self.batch_ids, real_slab_count);

        for slab_loc in slab_locs.into_iter() {
            debug!("submitting slab for finalization"; slab_loc);

            let batch = batches.next_batch();
            let world_ref = world_ref.clone();
            self.pool.submit(
                move || {
                    // need mutable world ref here to access the slab terrain mutably as we don't
                    // have it in scope here
                    let mut world = world_ref.borrow_mut();
                    let navigation = world
                        .process_inline_slab_terrain(slab_loc)
                        .expect("slab should be present");
                    drop(world);

                    // submit for finalization
                    Ok(LoadedSlab {
                        slab: slab_loc,
                        terrain: None, // owned by the world
                        navigation,
                        batch,
                    })
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

        self.last_batch_size = real_slab_count;
    }

    pub fn block_on_next_finalization(
        &mut self,
        timeout: Duration,
        bail: &impl Fn() -> bool,
    ) -> Option<Result<SlabLocation, TerrainSourceError>> {
        let end_time = Instant::now() + timeout;
        loop {
            if bail() {
                break Some(Err(TerrainSourceError::Bailed));
            }

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
        self.block_for_last_batch_with_bail(timeout, || false)
    }

    pub fn block_for_last_batch_with_bail(
        &mut self,
        timeout: Duration,
        bail: impl Fn() -> bool,
    ) -> Result<(), BlockForAllError> {
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
                    match self.block_on_next_finalization(timeout, &bail) {
                        None => return Err(BlockForAllError::TimedOut),
                        Some(Err(e)) => return Err(BlockForAllError::Error(e)),
                        Some(Ok(_)) => continue,
                    }
                }

                Ok(())
            }
        }
    }

    pub fn iter_occlusion_updates(&mut self, mut f: impl FnMut(OcclusionChunkUpdate)) {
        while let Ok(Some(update)) = self.chunk_updates_rx.try_next() {
            f(update)
        }
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
    use crate::loader::{AsyncWorkerPool, WorldLoader};
    use unit::world::SlabLocation;

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

        let mut loader =
            WorldLoader::<_, ()>::new(source, AsyncWorkerPool::new_blocking().unwrap());
        loader.request_slabs(vec![SlabLocation::new(1, (0, 0))].into_iter());

        let finalized = loader.block_on_next_finalization(Duration::from_secs(15), &|| false);
        assert_eq!(finalized.unwrap().unwrap(), SlabLocation::new(1, (0, 0)));

        assert_eq!(loader.world.borrow().all_chunks().count(), 1);
    }
}
