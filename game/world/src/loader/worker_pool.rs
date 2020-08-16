use std::time::Duration;

use crossbeam::channel::{unbounded, Receiver, Sender};
use threadpool::ThreadPool;

use common::*;
use unit::world::ChunkPosition;

use crate::chunk::ChunkTerrain;
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::{ChunkFinalizer, UpdateBatch};
use crate::{OcclusionChunkUpdate, WorldRef};
use std::collections::VecDeque;

pub type LoadTerrainResult = Result<(ChunkPosition, ChunkTerrain, UpdateBatch), TerrainSourceError>;

pub trait WorkerPool {
    fn start_finalizer(
        &mut self,
        world: WorldRef,
        finalize_rx: Receiver<LoadTerrainResult>,
        chunk_updates_tx: Sender<OcclusionChunkUpdate>,
    );

    fn block_on_next_finalize(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<ChunkPosition, TerrainSourceError>>;

    fn submit<T: 'static + Send + FnOnce() -> LoadTerrainResult>(
        &mut self,
        task: T,
        done_channel: Sender<LoadTerrainResult>,
    );
}

pub struct ThreadedWorkerPool {
    pool: ThreadPool,
    success_rx: Receiver<Result<ChunkPosition, TerrainSourceError>>,
    success_tx: Sender<Result<ChunkPosition, TerrainSourceError>>,
}

impl ThreadedWorkerPool {
    pub fn new(threads: usize) -> Self {
        let (success_tx, success_rx) = unbounded();
        Self {
            pool: ThreadPool::with_name("wrld-worker".to_owned(), threads),
            success_rx,
            success_tx,
        }
    }
}

impl WorkerPool for ThreadedWorkerPool {
    fn start_finalizer(
        &mut self,
        world: WorldRef,
        finalize_rx: Receiver<LoadTerrainResult>,
        chunk_updates_tx: Sender<OcclusionChunkUpdate>,
    ) {
        let success_tx = self.success_tx.clone();
        // TODO if this thread panics, propagate to main game thread
        std::thread::Builder::new()
            .name("wrld-finalize".to_owned())
            .spawn(move || {
                let mut finalizer = ChunkFinalizer::new(world, chunk_updates_tx);

                while let Ok(result) = finalize_rx.recv() {
                    let result = match result {
                        Err(e @ TerrainSourceError::OutOfBounds) => {
                            my_debug!("requested out of bounds chunk, no problem");
                            Err(e)
                        }
                        Err(e) => {
                            my_error!("failed to load requested chunk"; "error" => %e);
                            Err(e)
                        }
                        Ok(result) => {
                            let chunk = result.0;
                            finalizer.finalize(result);
                            Ok(chunk)
                        }
                    };

                    if let Err(e) = success_tx.send(result) {
                        my_error!("failed to report finalized chunk result"; "error" => %e);
                    }
                }

                // TODO detect this as an error condition?
                my_info!("terrain finalizer thread exiting")
            })
            .expect("finalizer thread failed to start");
    }

    fn block_on_next_finalize(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<ChunkPosition, TerrainSourceError>> {
        self.success_rx.recv_timeout(timeout).ok()
    }

    fn submit<T: 'static + Send + FnOnce() -> LoadTerrainResult>(
        &mut self,
        task: T,
        done_channel: Sender<LoadTerrainResult>,
    ) {
        self.pool.execute(move || {
            let result = task();

            // terrain has been processed in isolation on worker thread, now post to
            // finalization thread
            if let Err(e) = done_channel.send(result) {
                my_error!("failed to send terrain result to finalizer"; "error" => %e);
            }
        });
    }
}

#[derive(Default)]
pub struct BlockingWorkerPool {
    finalizer_magic: Option<(Receiver<LoadTerrainResult>, ChunkFinalizer)>,

    task_queue: VecDeque<(
        Box<dyn FnOnce() -> LoadTerrainResult>,
        Sender<LoadTerrainResult>,
    )>,
}

impl WorkerPool for BlockingWorkerPool {
    fn start_finalizer(
        &mut self,
        world: WorldRef,
        finalize_rx: Receiver<LoadTerrainResult>,
        chunk_updates_tx: Sender<OcclusionChunkUpdate>,
    ) {
        self.finalizer_magic = Some((finalize_rx, ChunkFinalizer::new(world, chunk_updates_tx)));
    }

    fn block_on_next_finalize(
        &mut self,
        _: Duration,
    ) -> Option<Result<ChunkPosition, TerrainSourceError>> {
        // time to actually do the work
        let (task, done_channel) = self.task_queue.pop_front()?;

        let (finalize_rx, finalizer) = self.finalizer_magic.as_mut().unwrap(); // set in start_finalizer

        // load chunk right here right now
        let result = task();

        // post to "finalizer thread"
        done_channel
            .send(result)
            .expect("failed to send to finalizer");

        // receive on "finalizer thread"
        let result = match finalize_rx
            .recv_timeout(Duration::from_secs(60))
            .expect("expected finalized terrain by now")
        {
            Err(e) => {
                my_error!("failed to load chunk"; "error" => %e);
                Err(e)
            }
            Ok(result) => {
                let chunk = result.0;

                // finalize on "finalizer thread"
                finalizer.finalize(result);
                Ok(chunk)
            }
        };

        // send back to "main thread"
        Some(result)
    }

    fn submit<T: 'static + Send + FnOnce() -> LoadTerrainResult>(
        &mut self,
        task: T,
        done_channel: Sender<LoadTerrainResult>,
    ) {
        // naaah, do the work later when we're asked for it
        self.task_queue.push_back((Box::new(task), done_channel));
    }
}
