use common::*;
use std::path::Path;

use resources::resource::Resources;
use simulation::{
    presets, GeneratedTerrainSource, Renderer, Simulation, ThreadedWorldLoader, WorkerPool,
    WorldLoader,
};
use std::time::Duration;

pub trait GamePreset<R: Renderer> {
    fn name(&self) -> &str;
    fn config(&self) -> Option<&Path> {
        None
    }
    fn world(&self) -> BoxedResult<ThreadedWorldLoader>;
    fn init(&self, sim: &mut Simulation<R>, scenario: Scenario) -> BoxedResult<()>;

    fn load(&self, resources: Resources, scenario: Scenario) -> BoxedResult<Simulation<R>> {
        let mut world = self.world()?;

        // TODO load initial slab range
        // debug!("waiting for world to load before initializing simulation");
        // world.request_all_chunks();
        // world.block_for_all(Duration::from_secs(30))?;
        todo!("load world");

        let mut sim = Simulation::new(world, resources)?;
        self.init(&mut sim, scenario)?;
        Ok(sim)
    }
}

mod ci;
mod dev;
mod empty;

use crate::scenarios::Scenario;
pub use ci::ContinuousIntegrationGamePreset;
pub use dev::DevGamePreset;
pub use empty::EmptyGamePreset;

fn world_from_source<D: 'static, P: WorkerPool<D>>(
    source: config::WorldSource,
    pool: P,
) -> Result<WorldLoader<P, D>, &'static str> {
    Ok(match source {
        config::WorldSource::Preset(preset) => {
            debug!("loading world preset"; "preset" => ?preset);
            let source = presets::from_preset(preset);
            WorldLoader::new(source, pool)
        }
        config::WorldSource::Generate { seed, radius } => {
            debug!("generating world with radius {radius}", radius = radius);
            let height_scale = config::get().world.generation_height_scale;
            let source = GeneratedTerrainSource::new(seed, radius, height_scale)?;
            WorldLoader::new(source, pool)
        }
    })
}
