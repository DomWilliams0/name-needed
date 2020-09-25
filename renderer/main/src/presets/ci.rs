use crate::presets::DevGamePreset;
use crate::GamePreset;
use common::BoxedResult;
use simulation::{
    presets, Renderer, Simulation, ThreadedWorkerPool, ThreadedWorldLoader, WorldLoader,
};
use std::path::Path;

#[derive(Default)]
pub struct ContinuousIntegrationGamePreset;

impl<R: Renderer> GamePreset<R> for ContinuousIntegrationGamePreset {
    fn name(&self) -> &str {
        "ci"
    }

    fn config(&self) -> Option<&Path> {
        Some(Path::new("ci_test.ron"))
    }

    fn world(&self) -> BoxedResult<ThreadedWorldLoader> {
        let pool = ThreadedWorkerPool::new(2);
        Ok(WorldLoader::new(presets::multi_chunk_wonder(), pool))
    }

    fn init(&self, sim: &mut Simulation<R>) -> BoxedResult<()> {
        // piggyback off dev preset
        DevGamePreset::default().init(sim)
    }
}
