use std::time::Duration;

use misc::*;
use unit::world::SlabLocation;

use crate::loader::terrain_source::TerrainSourceError;
use crate::{WorldContext, WorldRef};

use futures::channel::mpsc as async_channel;
use futures::{SinkExt, StreamExt};

use futures::executor::block_on;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

pub struct AsyncWorkerPool {
    pool: Runtime,
}

impl AsyncWorkerPool {
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
        let pool = builder.enable_time().build()?;
        Ok(Self { pool })
    }

    pub fn runtime(&self) -> &Runtime {
        &self.pool
    }
}
