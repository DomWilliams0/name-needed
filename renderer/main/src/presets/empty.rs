use crate::scenarios::Scenario;
use crate::GamePreset;
use common::BoxedResult;
use simulation::{
    presets, AsyncWorkerPool, Renderer, Simulation, ThreadedWorldLoader, WorldLoader,
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
        let pool = AsyncWorkerPool::new(1)?;
        Ok(WorldLoader::new(presets::multi_chunk_wonder(), pool))
    }

    fn init(&self, _: &mut Simulation<R>, _: Scenario) -> BoxedResult<()> {
        // create no entities
        Ok(())
    }
}
