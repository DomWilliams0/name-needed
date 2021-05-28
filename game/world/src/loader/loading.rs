use std::time::{Duration, Instant};

use common::*;
use futures::channel::mpsc as async_channel;
use unit::world::{ChunkLocation, GlobalSliceIndex, SlabIndex, SlabLocation, WorldPosition};

use crate::chunk::slab::{Slab, SlabInternalNavigability, SlabType};

use crate::loader::batch::UpdateBatchUniqueId;
use crate::loader::worker_pool::LoadTerrainResult;
use crate::world::{ContiguousChunkIterator, WorldChangeEvent};
use crate::{OcclusionChunkUpdate, WorldContext, WorldRef};

use crate::loader::terrain_source::BlockDetails;
use crate::loader::{
    AsyncWorkerPool, TerrainSource, TerrainSourceError, UpdateBatch, WorldTerrainUpdate,
};
use crate::world::slab_loading::SlabProcessingFuture;
use futures::FutureExt;
use std::collections::HashSet;
use std::iter::repeat;

pub struct WorldLoader<C: WorldContext> {
    source: TerrainSource,
    pool: AsyncWorkerPool,
    finalization_channel: async_channel::Sender<LoadTerrainResult>,
    chunk_updates_rx: async_channel::UnboundedReceiver<OcclusionChunkUpdate>,
    world: WorldRef<C>,
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

impl<C: WorldContext> WorldLoader<C> {
    pub fn new<S: Into<TerrainSource>>(source: S, mut pool: AsyncWorkerPool) -> Self {
        let (finalize_tx, finalize_rx) = async_channel::channel(16);
        let (chunk_updates_tx, chunk_updates_rx) = async_channel::unbounded();

        let world = WorldRef::default();
        pool.start_finalizer(world.clone(), finalize_rx, chunk_updates_tx);

        Self {
            source: source.into(),
            pool,
            finalization_channel: finalize_tx,
            chunk_updates_rx,
            world,
            last_batch_size: 0,
            batch_ids: UpdateBatchUniqueId::default(),
        }
    }

    pub fn world(&self) -> WorldRef<C> {
        self.world.clone()
    }

    /// Requests slabs as a single batch. Must be sorted as per [self.request_slabs_with_count]
    pub fn request_slabs(&mut self, slabs: impl ExactSizeIterator<Item = SlabLocation> + Clone) {
        let count = slabs.len();
        self.request_slabs_with_count(slabs, count)
    }

    // TODO add more efficient version that takes chunk+multiple slabs
    /// Must be sorted by chunk then by ascending slab (debug asserted). All slabs are loaded from
    /// scratch, it's the caller's responsibility to ensure slabs that are already loaded are not
    /// passed in here
    pub fn request_slabs_with_count(
        &mut self,
        slabs: impl Iterator<Item = SlabLocation> + Clone,
        count: usize,
    ) {
        // bomb out early if nothing to do
        if count == 0 {
            return;
        }

        let mut world_mut = self.world.borrow_mut();

        // check order of slabs is as expected
        if cfg!(debug_assertions) {
            let sorted = slabs
                .clone()
                .sorted_by(|a, b| a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab)));

            assert_equal(slabs.clone(), sorted);
        }

        let mut extra_slabs = SmallVec::<[SlabLocation; 16]>::new();

        // calculate chunk range
        let (mut chunk_min, mut chunk_max) = (ChunkLocation::MAX, ChunkLocation::MIN);

        // first iterate slabs to register them all with their chunk, so they know if their
        // neighbours have been requested/are being loaded too
        for (chunk_loc, slabs) in slabs.clone().group_by(|slab| slab.chunk).into_iter() {
            log_scope!(o!(chunk_loc));

            let chunk = world_mut.ensure_chunk(chunk_loc);

            // track the highest slab
            let mut highest = SlabIndex::MIN;

            for slab in slabs {
                chunk.mark_slab_requested(slab.slab);

                debug_assert!(slab.slab > highest, "slabs should be in ascending order");
                highest = slab.slab;
            }

            // should have seen some slabs
            assert_ne!(highest, SlabIndex::MIN);

            // request the slab above the highest as all-air if it's missing, so navigation
            // discovery works properly
            let empty = highest + 1;
            if !chunk.has_slab(empty) {
                extra_slabs.push(SlabLocation::new(empty, chunk.pos()));
                chunk.mark_slab_requested(empty);
            }

            // track chunk range
            chunk_min = chunk_min.min(chunk_loc);
            chunk_max = chunk_max.max(chunk_loc);
        }

        let count = count + extra_slabs.len();
        let mut batches = UpdateBatch::builder(&mut self.batch_ids, count);
        let mut real_count = 0;

        let all_slabs = {
            let real_slabs = slabs.zip(repeat(SlabType::Normal));
            let air_slabs = extra_slabs.into_iter().zip(repeat(SlabType::Placeholder));
            real_slabs.chain(air_slabs)
        };

        // let the terrain source know what's coming so it can kick off region generation
        {
            let source = self.source.clone();
            self.pool.submit_any_async_with_handle(async move {
                source.prepare_for_chunks((chunk_min, chunk_max)).await;
            });
        }

        for (slab, slab_type) in all_slabs {
            log_scope!(o!(slab));

            let source = self.source.clone();
            let batch = batches.next_batch();

            debug!(
                "submitting slab to pool as part of batch";
                slab, batch
            );

            // load raw terrain and do as much processing in isolation as possible on a worker thread
            let world = self.world();
            self.pool.submit_async(
                async move {
                    let result = if let SlabType::Placeholder = slab_type {
                        // empty placeholder
                        Ok(None)
                    } else {
                        source.load_slab(slab).await.map(Some)
                    };

                    let terrain = match result {
                        Ok(Some(terrain)) => terrain,
                        Ok(None) => {
                            debug!("adding placeholder slab to the top of the chunk"; slab);
                            Slab::empty_placeholder()
                        }
                        Err(TerrainSourceError::SlabOutOfBounds(slab)) => {
                            // soft error, we're at the world edge. treat as all air instead of
                            // crashing and burning
                            debug!("slab is out of bounds, swapping in an empty one"; slab);

                            // TODO shared instance of CoW for empty slab
                            Slab::empty_placeholder()
                        }
                        Err(err) => return Err(err),
                    };

                    // slab terrain is now fixed, process it concurrently on a worker thread.
                    // may require waiting for another slab to finish, and world lock must be
                    // released during the wait to prevent a deadlock.
                    let (terrain, navigation) =
                        SlabProcessingFuture::with_provided_terrain(world, slab, terrain)
                            .await
                            .expect("chunk should be present");

                    assert!(terrain.is_some(), "slab ownership expected");

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
    pub fn apply_terrain_updates(
        &mut self,
        terrain_updates: &mut HashSet<WorldTerrainUpdate>,
        changes_out: &mut Vec<WorldChangeEvent>,
    ) {
        let world_ref = self.world.clone();

        let (slab_updates, upper_slab_limit) = {
            // translate world -> slab updates, preserving original mapping
            // TODO reuse vec allocs
            let mut slab_updates = terrain_updates
                .iter()
                .cloned()
                .flat_map(|world_update| {
                    world_update
                        .clone()
                        .into_slab_updates()
                        .map(move |update| (world_update.clone(), update))
                })
                .collect_vec();
            let mut slab_updates_to_keep = Vec::with_capacity(slab_updates.len());

            // sort then group by chunk and slab, so each slab is touched only once
            slab_updates.sort_unstable_by(|(_, (a, _)), (_, (b, _))| {
                a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab))
            });

            let world = world_ref.borrow();
            let mut chunks_iter = ContiguousChunkIterator::new(&*world);
            for (slab, updates) in &slab_updates.into_iter().group_by(|(_, (slab, _))| *slab) {
                enum UpdateApplication {
                    /// Pop updates from set and apply now
                    Apply,
                    /// Don't apply now, defer until another tick
                    Defer,
                    /// Don't apply ever, remove from set
                    Drop,
                }

                let application = match chunks_iter.next(slab.chunk) {
                    Some(chunk) => {
                        if chunk.is_slab_loaded(slab.slab) {
                            UpdateApplication::Apply
                        } else {
                            UpdateApplication::Defer
                        }
                    }
                    None => UpdateApplication::Drop,
                };

                match application {
                    UpdateApplication::Apply => {
                        // updates to be applied now
                        slab_updates_to_keep.extend(updates.into_iter().map(
                            |(original, update)| {
                                // remove from update set
                                terrain_updates.remove(&original);

                                // remove now unnecessary original mapping from update
                                update
                            },
                        ));
                    }

                    UpdateApplication::Defer => {
                        if cfg!(debug_assertions) {
                            let count = updates.count();
                            trace!("deferring {count} terrain updates for slab because it's currently loading", count = count; slab.chunk);
                        } else {
                            // avoid consuming expensive iterator when not logging
                            trace!("deferring terrain updates for slab because it's currently loading"; slab.chunk);
                        };
                    }
                    UpdateApplication::Drop => {
                        // remove from update set
                        let mut count = 0;
                        for (orig, _) in updates.dedup_by(|(a, _), (b, _)| *a == *b) {
                            terrain_updates.remove(&orig);
                            count += 1;
                        }

                        debug!("dropping {count} terrain updates for chunk because it's not loaded", count = count; slab.chunk);
                    }
                };
            }

            // count slabs for vec allocation, upper limit because some might be filtered out.
            // no allocations in dedup because vec is sorted
            let upper_slab_limit = slab_updates_to_keep
                .iter()
                .dedup_by(|(a, _), (b, _)| a == b)
                .count();

            (slab_updates_to_keep, upper_slab_limit)
        };

        if upper_slab_limit == 0 {
            // nothing to do
            return;
        }

        // group per slab so each slab is fetched and modified only once
        let grouped_updates = slab_updates.into_iter().group_by(|(slab, _)| *slab);
        let grouped_updates = grouped_updates
            .into_iter()
            .map(|(slab, updates)| (slab, updates.map(|(_, update)| update)));

        // modify slabs in place - even though the changes won't be fully visible in the game yet (in terms of
        // navigation or rendering), world queries in the next game tick will be current with the
        // changes applied now.
        // TODO reuse buf
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

            self.pool.submit_async(
                async move {
                    let (_, navigation) =
                        SlabProcessingFuture::with_inline_terrain(world_ref, slab_loc)
                            .await
                            .expect("chunk and slab should be present");

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
        match std::mem::take(&mut self.last_batch_size) {
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

    pub fn get_ground_level(
        &self,
        block: WorldPosition,
    ) -> Result<GlobalSliceIndex, TerrainSourceError> {
        let fut = self.source.get_ground_level(block);
        self.pool.runtime().block_on(fut)
    }

    pub fn query_block(&self, block: WorldPosition) -> Option<BlockDetails> {
        let fut = self.source.query_block(block);
        self.pool.runtime().block_on(fut)
    }

    pub fn is_generated(&self) -> bool {
        matches!(self.source, TerrainSource::Generated(_))
    }

    /// Nop if any mutexes cannot be taken immediately
    pub fn feature_boundaries_in_range(
        &self,
        chunks: impl Iterator<Item = ChunkLocation>,
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        per_point: impl FnMut(u64, WorldPosition),
    ) {
        let fut = self
            .source
            .feature_boundaries_in_range(chunks, z_range, per_point);
        let _ = fut.now_or_never();
    }

    pub fn steal_queued_block_updates(&self, out: &mut HashSet<WorldTerrainUpdate>) {
        let fut = self.source.steal_queued_block_updates(out);
        self.pool.runtime().block_on(fut)
    }
}

#[cfg(test)]
mod tests {

    use std::time::Duration;

    use unit::world::{
        all_slabs_in_range, ChunkLocation, SlabPosition, WorldPosition, WorldPositionRange,
        CHUNK_SIZE,
    };

    use crate::block::BlockType;
    use crate::chunk::ChunkBuilder;
    use crate::helpers::test_world_timeout;
    use crate::loader::loading::WorldLoader;
    use crate::loader::terrain_source::MemoryTerrainSource;
    use crate::loader::{AsyncWorkerPool, WorldTerrainUpdate};
    use crate::world::helpers::DummyWorldContext;
    use crate::BaseTerrain;
    use common::{Itertools, Rng, SeedableRng, SliceRandom, SmallRng};
    use std::collections::{HashMap, HashSet};
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
            WorldLoader::<DummyWorldContext>::new(source, AsyncWorkerPool::new_blocking().unwrap());
        loader.request_slabs(vec![SlabLocation::new(1, (0, 0))].into_iter());

        let finalized = loader.block_on_next_finalization(Duration::from_secs(15), &|| false);
        assert_eq!(finalized.unwrap().unwrap(), SlabLocation::new(1, (0, 0)));

        assert_eq!(loader.world.borrow().all_chunks().count(), 1);
    }

    #[test]
    #[ignore]
    /// Ensure block updates are applied as expected when stressed. Came out of debugging a race
    /// condition when applying terrain updates while a chunk is being finalized, but didn't actually
    /// help to reproduce it! Keeping it around as a regression test anyway
    fn block_updates_sanity_check() {
        const WORLD_SIZE: i32 = 8;
        const UPDATE_COUNT: usize = 1000;
        const BATCH_SIZE_RANGE: (usize, usize) = (5, 200);
        const Z_RANGE: i32 = 8;

        // common::logging::for_tests();
        let source = {
            let chunks = (-WORLD_SIZE..WORLD_SIZE)
                .cartesian_product(-WORLD_SIZE..WORLD_SIZE)
                .map(|pos| (pos, ChunkBuilder::new().into_inner()));
            MemoryTerrainSource::from_chunks(chunks).unwrap()
        };

        let slabs_to_load = all_slabs_in_range(
            SlabLocation::new(-Z_RANGE, (-WORLD_SIZE, -WORLD_SIZE)),
            SlabLocation::new(Z_RANGE, (WORLD_SIZE, WORLD_SIZE)),
        )
        .0
        .collect_vec();

        let mut loader = WorldLoader::<DummyWorldContext>::new(
            source,
            AsyncWorkerPool::new(num_cpus::get()).unwrap(),
        );

        // create block updates before requesting slabs so there's no wait
        let mut rando = SmallRng::from_entropy();
        let blocks_to_set = {
            let block_types = vec![BlockType::Stone, BlockType::Dirt];

            // set each block once only
            (0..UPDATE_COUNT)
                .map(|_| {
                    const XY_RANGE: i32 = WORLD_SIZE * CHUNK_SIZE.as_i32();
                    let x = rando.gen_range(-XY_RANGE, XY_RANGE);
                    let y = rando.gen_range(-XY_RANGE, XY_RANGE);
                    let z = rando.gen_range(-Z_RANGE, Z_RANGE);
                    let pos = WorldPosition::from((x, y, z));
                    let block_type = block_types.choose(&mut rando).unwrap().to_owned();

                    (pos, block_type)
                })
                .collect::<HashMap<_, _>>()
        };
        // prepare update batches
        let mut update_batches = {
            let mut all = blocks_to_set
                .iter()
                .map(|(pos, block)| {
                    WorldTerrainUpdate::new(WorldPositionRange::with_single(*pos), *block)
                })
                .collect_vec();
            // could do this without so many allocs but it doesn't matter

            let mut batches = vec![];
            while !all.is_empty() {
                let (min, max) = BATCH_SIZE_RANGE;
                let n = rando.gen_range(min, max).min(all.len());
                let batch = all.drain(0..n).collect::<HashSet<_>>();
                batches.push(batch);
            }

            batches
        };
        let mut _changes = Vec::with_capacity(blocks_to_set.len());

        // load all slabs and wait for them to be present, otherwise the terrain updates are dropped
        loader.request_slabs(slabs_to_load.into_iter());
        assert!(
            loader.block_for_last_batch(test_world_timeout()).is_ok(),
            "timed out waiting for initial world finalization"
        );

        while !update_batches.is_empty() {
            let mut batch = update_batches.pop().unwrap(); // not empty
            let log_str = batch.iter().map(|x| format!("{:?}", x)).join("\n"); // gross
            common::trace!(
                "test: requesting batch of {} updates {}",
                batch.len(),
                log_str
            );
            loader.apply_terrain_updates(&mut batch, &mut _changes);
            if !batch.is_empty() {
                // push to back of "queue"
                update_batches.insert(0, batch);
            }
        }

        // wait for everything to settle down
        let timeout = test_world_timeout();
        loop {
            common::info!(
                "test: waiting {:?} for world to settle down",
                timeout.min(Duration::from_secs(10))
            );
            let _ = loader.block_for_last_batch(timeout); // idk block longer
            if loader
                .block_on_next_finalization(timeout, &|| false)
                .is_none()
            {
                // timed out
                break;
            }

            // consume updates to keep memory down
            loader.iter_occlusion_updates(|_| {});
        }

        let world = loader.world();
        let world = world.borrow();

        // collect all non air blocks in the world
        let set_blocks = {
            let mut actual_blocks = vec![];
            let mut chunk_blocks = vec![];
            for chunk in world.all_chunks() {
                let blocks = chunk.blocks(&mut chunk_blocks);
                for (block_pos, block) in blocks.drain(..) {
                    let block_type = block.block_type();

                    if !matches!(block_type, BlockType::Air) {
                        let world_pos = block_pos.to_world_position(chunk.pos());
                        actual_blocks.push((world_pos, block_type));
                    }
                }
            }

            actual_blocks
        };

        fn log_block(pos: WorldPosition) {
            let slab = SlabLocation::new(pos.slice(), ChunkLocation::from(pos));
            let block = SlabPosition::from(pos);
            eprintln!("test: btw {} is {:?} in slab {}", pos, block, slab);
        }

        // ensure the only non-air blocks are the ones we set
        for (pos, ty) in set_blocks {
            log_block(pos);
            match blocks_to_set.get(&pos) {
                None => panic!(
                    "unexpected block set at {:?}, should be air but is {:?}",
                    pos, ty
                ),
                Some(expected_ty) => {
                    assert_eq!(
                        *expected_ty, ty,
                        "block at {:?} should be {:?} but is actually {:?}",
                        pos, expected_ty, ty
                    );
                }
            }
        }

        // ensure all the blocks we set are non-air
        for (pos, expected_ty) in blocks_to_set.iter() {
            log_block(*pos);
            match world.block(*pos) {
                None => panic!("expected block {:?} does not exist", pos),
                Some(b) => {
                    let ty = b.block_type();
                    if let BlockType::Air = ty {
                        panic!(
                            "block at {:?} is unset but should been set to {:?}",
                            pos, ty
                        );
                    } else {
                        assert_eq!(
                            *expected_ty, ty,
                            "block at {:?} should have been set to {:?} but is actually {:?}",
                            pos, expected_ty, ty
                        );
                    }
                }
            }
        }
    }
}
