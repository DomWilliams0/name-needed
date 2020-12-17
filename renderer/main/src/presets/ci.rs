use crate::presets::{world_from_source, DevGamePreset};
use crate::scenarios::Scenario;
use crate::GamePreset;
use common::BoxedResult;
use simulation::{AsyncWorkerPool, Renderer, Simulation, ThreadedWorldLoader};
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

    fn world(&self, resources: &resources::WorldGen) -> BoxedResult<ThreadedWorldLoader> {
        let pool = AsyncWorkerPool::new(2)?;
        let which_source = config::get().world.source.clone();
        world_from_source(which_source, pool, resources)
    }

    fn init(&self, sim: &mut Simulation<R>, scenario: Scenario) -> BoxedResult<()> {
        // piggyback off dev preset
        DevGamePreset::default().init(sim, scenario)
    }
}
