use crate::GamePreset;
use common::BoxedResult;
use simulation::{
    presets, Renderer, Simulation, ThreadedWorkerPool, ThreadedWorldLoader, WorldLoader,
};
use std::path::Path;

#[derive(Default)]
pub struct EmptyGamePreset;

impl<R: Renderer> GamePreset<R> for EmptyGamePreset {
    fn name(&self) -> &str {
        "empty"
    }

    fn config(&self) -> Option<&Path> {
        Some(Path::new("config.ron"))
    }

    fn world(&self) -> BoxedResult<ThreadedWorldLoader> {
        let pool = ThreadedWorkerPool::new(1);
        Ok(WorldLoader::new(presets::multi_chunk_wonder(), pool))
    }

    fn init(&self, _sim: &mut Simulation<R>) -> BoxedResult<()> {
        // nop
        Ok(())
    }
}
