use std::time::Duration;

use common::*;
use unit::world::SlabLocation;

use crate::loader::finalizer::SlabFinalizer;
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::LoadedSlab;
use crate::{OcclusionChunkUpdate, WorldRef};

use futures::channel::mpsc as async_channel;
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicUsize, Ordering};

pub type LoadTerrainResult = Result<LoadedSlab, TerrainSourceError>;

pub struct AsyncWorkerPool {
    pool: tokio::runtime::Runtime,
    success_rx: async_channel::UnboundedReceiver<Result<SlabLocation, TerrainSourceError>>,
    success_tx: async_channel::UnboundedSender<Result<SlabLocation, TerrainSourceError>>,
}

impl AsyncWorkerPool {
    /// Spawns no threads, only runs on current thread
    pub fn new_blocking() -> Result<Self, futures::io::Error> {
        Self::with_rt_builder(tokio::runtime::Builder::new_current_thread())
    }

    /// Runs tasks on a thread pool
    pub fn new(threads: usize) -> Result<Self, futures::io::Error> {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder
            .worker_threads(threads)
            .max_threads(threads)
            .thread_name_fn(|| {
                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("wrld-worker-{}", id)
            });
        Self::with_rt_builder(builder)
    }

    fn with_rt_builder(mut builder: tokio::runtime::Builder) -> Result<Self, futures::io::Error> {
        let (success_tx, success_rx) = async_channel::unbounded();
        let pool = builder.enable_time().build()?;
        Ok(Self {
            pool,
            success_rx,
            success_tx,
        })
    }

    pub fn start_finalizer<D: 'static>(
        &mut self,
        world: WorldRef<D>,
        mut finalize_rx: async_channel::Receiver<LoadTerrainResult>,
        chunk_updates_tx: async_channel::UnboundedSender<OcclusionChunkUpdate>,
    ) {
        let mut success_tx = self.success_tx.clone();
        // TODO prioritize finalizer task - separate OS thread or runtime?
        self.pool.spawn(async move {
            let mut finalizer = SlabFinalizer::new(world, chunk_updates_tx);

            while let Some(result) = finalize_rx.next().await {
                let result = match result {
                    Err(e) => {
                        error!("failed to load requested slab"; "error" => %e);
                        Err(e)
                    }
                    Ok(result) => {
                        let slab = result.slab;
                        finalizer.finalize(result);
                        Ok(slab)
                    }
                };

                if let Err(e) = success_tx.send(result).await {
                    error!("failed to report finalized terrain result"; "error" => %e);
                    // trace!("lost result"; "result" => ?e.0);
                }
            }

            // TODO detect this as an error condition?
            info!("terrain finalizer thread exiting")
        });
    }

    pub fn block_on_next_finalize(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<SlabLocation, TerrainSourceError>> {
        let pool = &self.pool;
        let rx = &mut self.success_rx;
        pool.block_on(async {
            let future = rx.next();
            tokio::time::timeout(timeout, future)
                .await
                .unwrap_or_default()
        })
    }

    pub fn submit<T: 'static + Send + FnOnce() -> LoadTerrainResult>(
        &mut self,
        task: T,
        mut done_channel: async_channel::Sender<LoadTerrainResult>,
    ) {
        self.pool.spawn(async move {
            let result = task();

            // terrain has been processed in isolation on worker thread, now post to
            // finalization thread
            if let Err(e) = done_channel.send(result).await {
                error!("failed to send terrain result to finalizer"; "error" => %e);
            }
        });
    }
}
