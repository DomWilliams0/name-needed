use common::*;
use std::path::Path;

use resources::{ResourceContainer, Resources};
use simulation::{
    all_slabs_in_range, presets, AsyncWorkerPool, GeneratedTerrainSource, PlanetParams, Renderer,
    Simulation, SlabLocation, ThreadedWorldLoader, WorldLoader,
};
use std::time::Duration;

pub trait GamePreset<R: Renderer> {
    fn name(&self) -> &str;
    fn config(&self) -> Option<&Path> {
        None
    }
    fn world(&self, resources: &resources::WorldGen) -> BoxedResult<ThreadedWorldLoader>;
    fn init(&self, sim: &mut Simulation<R>, scenario: Scenario) -> BoxedResult<()>;

    fn load(&self, resources: Resources, scenario: Scenario) -> BoxedResult<Simulation<R>> {
        let mut world = self.world(&resources.world_gen()?)?;

        // TODO get initial slab range to request from engine
        let ((min_x, min_y, min_z), (max_x, max_y, max_z)) = config::get().world.initial_slab_range;
        debug!("waiting for world to load before initializing simulation");
        let (slabs_to_request, slab_count) = all_slabs_in_range(
            SlabLocation::new(min_z, (min_x, min_y)),
            SlabLocation::new(max_z, (max_x, max_y)),
        );

        world.request_slabs_with_count(slabs_to_request, slab_count);
        world.block_for_last_batch_with_bail(Duration::from_secs(30), panic::has_panicked)?;

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
use common::panic;
pub use dev::DevGamePreset;
pub use empty::EmptyGamePreset;

fn world_from_source(
    source: config::WorldSource,
    pool: AsyncWorkerPool,
    resources: &resources::WorldGen,
) -> BoxedResult<WorldLoader<simulation::WorldContext>> {
    Ok(match source {
        config::WorldSource::Preset(preset) => {
            debug!("loading world preset"; "preset" => ?preset);
            let source = presets::from_preset(preset);
            WorldLoader::new(source, pool)
        }
        config::WorldSource::Generate(file_path) => {
            let config_res = resources.get_file(file_path)?;
            debug!("generating world from config"; "path" => %config_res.display());

            let params = PlanetParams::load_with_only_file(config_res);
            let source = params.and_then(GeneratedTerrainSource::new)?;
            WorldLoader::new(source, pool)
        }
    })
}
