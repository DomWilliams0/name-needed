use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam::channel::{Receiver, Sender};
use crossbeam::crossbeam_channel::{bounded, unbounded};

use common::*;
pub use terrain_source::TerrainSource;
pub use terrain_source::{GeneratedTerrainSource, MemoryTerrainSource};
use unit::world::ChunkPosition;

pub use update::{ChunkTerrainUpdate, WorldTerrainUpdate};
pub use worker_pool::{BlockingWorkerPool, ThreadedWorkerPool, WorkerPool};

use crate::chunk::{BaseTerrain, Chunk, ChunkTerrain, RawChunkTerrain, WhichChunk};
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::worker_pool::LoadTerrainResult;
use crate::navigation::AreaNavEdge;
use crate::occlusion::NeighbourOffset;
use crate::{OcclusionChunkUpdate, WorldRef};

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
}

struct ChunkFinalizer {
    world: WorldRef,
    updates: Sender<OcclusionChunkUpdate>,
}

#[derive(Debug)]
pub enum BlockForAllResult {
    /// Terrain source needs to have had `request_all` called to know how many to wait for
    Unsupported,
    Success,
    TimedOut,
    Error(TerrainSourceError),
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
        }
    }

    pub fn world(&self) -> WorldRef {
        self.world.clone()
    }

    pub fn request_all_chunks(&mut self) {
        let chunks = self.source.lock().unwrap().all_chunks();
        let mut all_count = 0;
        chunks.iter().for_each(|&c| {
            self.request_chunk(c);
            all_count += 1;
        });

        self.all_count = Some(all_count);
    }

    pub fn request_chunk(&mut self, chunk: ChunkPosition) {
        // TODO cache full finalized chunks

        let source = self.source.clone();

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
                let terrain = ChunkTerrain::from_raw_terrain(terrain, chunk);
                Ok((chunk, terrain))
            },
            self.finalization_channel.clone(),
        );
    }

    pub fn update_chunk(&mut self, chunk: ChunkPosition, terrain: RawChunkTerrain) {
        self.pool.submit(
            move || {
                // concurrently process raw terrain into chunk terrain
                let terrain = ChunkTerrain::from_raw_terrain(terrain, chunk);
                Ok((chunk, terrain))
            },
            self.finalization_channel.clone(),
        )
    }

    pub fn block_on_next_finalization(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<ChunkPosition, TerrainSourceError>> {
        self.pool.block_on_next_finalize(timeout)
    }

    pub fn block_for_all(&mut self, timeout: Duration) -> BlockForAllResult {
        match self.all_count {
            None => BlockForAllResult::Unsupported,
            Some(count) => {
                let start_time = Instant::now();
                for i in 0..count {
                    let elapsed = start_time.elapsed();
                    let timeout = match timeout.checked_sub(elapsed) {
                        None => return BlockForAllResult::TimedOut,
                        Some(t) => t,
                    };

                    trace!("waiting for chunk {}/{} for {:?}", (i + 1), count, timeout);
                    match self.block_on_next_finalization(timeout) {
                        None => return BlockForAllResult::TimedOut,
                        Some(Err(e)) => return BlockForAllResult::Error(e),
                        Some(Ok(_)) => continue,
                    }
                }

                BlockForAllResult::Success
            }
        }
    }

    pub fn chunk_updates_rx(&mut self) -> Option<Receiver<OcclusionChunkUpdate>> {
        self.chunk_updates_rx.take()
    }
}

impl ChunkFinalizer {
    fn new(world: WorldRef, updates: Sender<OcclusionChunkUpdate>) -> Self {
        Self { world, updates }
    }

    fn finalize(&mut self, (chunk, mut terrain): (ChunkPosition, ChunkTerrain)) {
        // world lock is taken and released often to prevent holding up the main thread

        for (direction, offset) in NeighbourOffset::offsets() {
            let world = self.world.borrow();

            let neighbour_offset = chunk + offset;
            let neighbour_terrain = match world
                .find_chunk_with_pos(neighbour_offset)
                .map(|c| c.raw_terrain())
            {
                Some(terrain) => terrain,
                // chunk is not loaded
                None => continue,
            };

            // TODO reuse/pool bufs
            let mut this_terrain_updates = Vec::new();
            let mut other_terrain_updates = Vec::new();

            terrain.raw_terrain().cross_chunk_pairs_foreach(
                neighbour_terrain,
                direction,
                |which, block_pos, opacity| {
                    match which {
                        WhichChunk::ThisChunk => {
                            // update opacity now for this chunk being loaded
                            this_terrain_updates.push((block_pos, opacity));
                        }
                        WhichChunk::OtherChunk => {
                            other_terrain_updates.push((block_pos, opacity));
                        }
                    }
                },
            );

            // apply opacity changes to this chunk now
            this_terrain_updates
                .drain(..)
                .for_each(|(block_pos, opacity)| {
                    terrain
                        .raw_terrain_mut()
                        .with_block_mut_unchecked(block_pos, |b| {
                            b.occlusion_mut().update_from_neighbour_opacities(opacity);
                        });
                });

            // queue changes to existing chunks in world
            self.updates
                .send(OcclusionChunkUpdate(
                    neighbour_offset,
                    other_terrain_updates,
                ))
                .unwrap();
        }

        // navigation
        let mut area_edges = Vec::new(); // TODO reuse buf
        for (direction, offset) in NeighbourOffset::aligned() {
            let world = self.world.borrow();

            let neighbour_offset = chunk + offset;
            let neighbour_terrain = match world
                .find_chunk_with_pos(neighbour_offset)
                .map(|c| c.raw_terrain())
            {
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

#[cfg(test)]
mod tests {
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
        loader.request_chunk(ChunkPosition(0, 0));

        let finalized = loader.block_on_next_finalization(Duration::from_secs(15));
        assert_matches!(finalized, Some(Ok(ChunkPosition(0, 0))));

        assert_eq!(loader.world.borrow().all_chunks().count(), 1);
    }
}
