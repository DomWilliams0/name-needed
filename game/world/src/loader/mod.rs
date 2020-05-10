use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam::channel::{Receiver, Sender};
use crossbeam::crossbeam_channel::unbounded;

use common::*;
pub(crate) use terrain_source::MemoryTerrainSource;
pub use terrain_source::TerrainSource;
use unit::world::{BlockPosition, ChunkPosition};

#[cfg(test)]
pub use worker_pool::BlockingWorkerPool;
pub use worker_pool::{ThreadedWorkerPool, WorkerPool};

use crate::navigation::AreaNavEdge;
use crate::chunk::{BaseTerrain, Chunk, ChunkTerrain, WhichChunk};
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::worker_pool::LoadTerrainResult;
use crate::occlusion::{NeighbourOffset, NeighbourOpacity};
use crate::WorldRef;

mod terrain_source;
mod worker_pool;

pub type ChunkUpdate = (ChunkPosition, Vec<(BlockPosition, NeighbourOpacity)>);

pub struct WorldLoader<P: WorkerPool> {
    source: Arc<Mutex<dyn TerrainSource>>,
    pool: P,
    finalization_channel: Sender<LoadTerrainResult>,
    chunk_updates_rx: Option<Receiver<ChunkUpdate>>,
    world: WorldRef,
    all_count: Option<usize>,
}

struct ChunkFinalizer {
    world: WorldRef,
    updates: Sender<ChunkUpdate>,
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
        let (finalize_tx, finalize_rx) = unbounded();
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
                let terrain = {
                    let mut terrain_source = source.lock().unwrap();

                    // quick validity check
                    if !terrain_source.is_in_bounds(chunk) {
                        return Err(TerrainSourceError::OutOfBounds);
                    }

                    // load raw terrain NOT in parallel (reading from a file etc)
                    terrain_source.load_chunk(chunk)?
                };

                // concurrently process in isolation
                let terrain = ChunkTerrain::from_raw_terrain(terrain, chunk);
                Ok((chunk, terrain))
            },
            self.finalization_channel.clone(),
        );
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

    pub fn chunk_updates_rx(&mut self) -> Option<Receiver<ChunkUpdate>> {
        self.chunk_updates_rx.take()
    }
}

impl ChunkFinalizer {
    fn new(world: WorldRef, updates: Sender<ChunkUpdate>) -> Self {
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
                .send((neighbour_offset, other_terrain_updates))
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
                    // debug!("{:?} link from chunk {:?} area {:?} ---------> chunk {:?} area {:?} ============= direction {:?} {}",
                    //       edge_cost, chunk, from_area, neighbour_offset, to_area, direction, i);

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

        // TODO build up area graph nodes and edges (using a map in self of all loaded chunks->edge opacity/walkability?)

        // TODO finally take WorldRef write lock and 1) update nav graph 2) add chunk

        let chunk = Chunk::with_completed_terrain(chunk, terrain);
        {
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
