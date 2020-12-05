use common::*;
use std::path::Path;

use resources::resource::Resources;
use simulation::{
    all_slabs_in_range, presets, GeneratedTerrainSource, Renderer, Simulation, SlabLocation,
    ThreadedWorldLoader, WorkerPool, WorldLoader,
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
pub use dev::DevGamePreset;
pub use empty::EmptyGamePreset;
use engine::panic;

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
