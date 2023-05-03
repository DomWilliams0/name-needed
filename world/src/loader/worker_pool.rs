use std::time::Duration;

use misc::*;
use unit::world::SlabLocation;

use crate::loader::terrain_source::TerrainSourceError;
use crate::{OcclusionChunkUpdate, WorldContext, WorldRef};

use crate::loader::loading::LoadedSlab;
use futures::channel::mpsc as async_channel;
use futures::{SinkExt, StreamExt};

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

pub type LoadTerrainResult = Result<SlabLocation, TerrainSourceError>;

pub struct AsyncWorkerPool {
    pool: tokio::runtime::Runtime,
    success_rx: async_channel::UnboundedReceiver<Result<SlabLocation, TerrainSourceError>>,
    success_tx: LoadSuccessTx,
}

pub type LoadSuccessTx = async_channel::UnboundedSender<Result<SlabLocation, TerrainSourceError>>;

impl AsyncWorkerPool {
    /// Spawns no threads, only runs on current thread
    pub fn new_blocking() -> Result<Self, futures::io::Error> {
        Self::with_rt_builder(tokio::runtime::Builder::new_current_thread())
    }

    /// Runs tasks on a thread pool
    pub fn new(threads: usize) -> Result<Self, futures::io::Error> {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.worker_threads(threads).thread_name_fn(|| {
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

    pub(in crate::loader) fn success_tx(
        &self,
    ) -> async_channel::UnboundedSender<Result<SlabLocation, TerrainSourceError>> {
        // TODO should this even live here?
        self.success_tx.clone()
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

    pub fn submit_async<R: Send + 'static>(
        &mut self,
        task: impl Future<Output = R> + Send + 'static,
        mut done_channel: async_channel::UnboundedSender<R>,
    ) {
        self.pool.spawn(async move {
            let result = task.await;
            if let Err(e) = done_channel.send(result).await {
                error!("failed to send terrain result"; "error" => %e);
            }
        });
    }

    pub fn submit_any_async_with_handle<R: Send + 'static>(
        &self,
        task: impl Future<Output = R> + Send + 'static,
    ) -> JoinHandle<R> {
        self.pool.spawn(async move { task.await })
    }

    // async fn send_result<R>(
    //     mut done_channel: async_channel::Sender<R>,
    //     result: R,
    // ) {
    //     // terrain has been processed in isolation on worker thread, now post to
    //     // finalization thread
    // }

    pub fn runtime(&self) -> &Runtime {
        &self.pool
    }
}
