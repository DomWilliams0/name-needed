use common::*;
use std::path::Path;

use resources::resource::Resources;
use simulation::{
    presets, GeneratedTerrainSource, Renderer, Simulation, SlabLocation, ThreadedWorldLoader,
    WorkerPool, WorldLoader,
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
        debug!("waiting for world to load before initializing simulation");
        let (slabs_to_request, slab_count) = {
            let chunk_range = ((-1, -1), (1, 1));
            let slab_range = (-1, 1);

            let chunks = ((chunk_range.0).0..=(chunk_range.1).0)
                .cartesian_product((chunk_range.0).1..=(chunk_range.1).1);
            let all_slabs = chunks
                .flat_map(move |chunk| {
                    let slabs = slab_range.0..=slab_range.1;
                    slabs.zip(repeat(chunk))
                })
                .map(|(slab, chunk)| SlabLocation::new(slab, chunk));

            let slab_count = slab_range.1 - slab_range.0 + 1;
            let chunk_count = ((chunk_range.1).0 - (chunk_range.0).0 + 1)
                * ((chunk_range.1).1 - (chunk_range.0).1 + 1);

            (all_slabs, slab_count as usize * chunk_count as usize)
        };

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
use std::iter::repeat;

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
