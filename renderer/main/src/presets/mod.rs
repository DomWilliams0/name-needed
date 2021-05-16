use common::*;

use resources::Resources;
use simulation::{
    all_slabs_in_range, presets, AsyncWorkerPool, ChunkLocation, GeneratedTerrainSource,
    PlanetParams, Renderer, Simulation, SlabLocation, TerrainSourceError, ThreadedWorldLoader,
    WorldLoader, WorldPosition,
};
use std::time::Duration;

pub trait GamePreset<R: Renderer> {
    fn name(&self) -> &str;
    fn config_filename(&self) -> Option<&str> {
        None
    }
    fn world(&self, resources: &resources::WorldGen) -> BoxedResult<ThreadedWorldLoader>;
    fn init(&self, sim: &mut Simulation<R>, scenario: Scenario) -> BoxedResult<()>;

    /// (_, block to initially centre renderer on)
    fn load(
        &self,
        resources: Resources,
        scenario: Scenario,
    ) -> BoxedResult<(Simulation<R>, WorldPosition)> {
        let mut world = self.world(&resources.world_gen()?)?;

        let (chunk, slab_depth, chunk_radius, is_preset) = {
            let cfg = config::get();
            let (cx, cy) = match cfg.world.source {
                WorldSource::Preset(_) => (0, 0), // ignore config for preset worlds
                WorldSource::Generate(_) => cfg.world.initial_chunk,
            };

            (
                ChunkLocation(cx, cy),
                cfg.world.initial_slab_depth as i32,
                cfg.world.initial_chunk_radius as i32,
                cfg.world.source.is_preset(),
            )
        };

        // request ground level in requested start chunk
        // TODO middle of requested chunk instead of corner
        let ground_level = {
            let block = chunk.get_block(0); // z ignored
            match world.get_ground_level(block) {
                Ok(slice) => slice,
                Err(TerrainSourceError::BlockOutOfBounds(_)) if is_preset => {
                    // special case, assume preset starts at 0
                    warn!(
                        "could not find block {:?} in preset world, assuming ground is at 0",
                        block
                    );
                    0.into()
                }
                err => err?,
            }
        };

        let ground_slab = ground_level.slab_index();

        debug!(
            "ground level in {chunk:?} is {ground}",
            chunk = chunk,
            ground = ground_level.slice();
            ground_slab,
        );

        let initial_block = chunk.get_block(ground_level);
        info!("centring camera on block"; "block" => %initial_block);

        let (slabs_to_request, slab_count) = all_slabs_in_range(
            SlabLocation::new(
                ground_slab - slab_depth,
                (chunk.x() - chunk_radius, chunk.y() - chunk_radius),
            ),
            SlabLocation::new(
                ground_slab + slab_depth,
                (chunk.x() + chunk_radius, chunk.y() + chunk_radius),
            ),
        );

        debug!(
            "waiting for world to load {slabs} slabs around chunk {chunk:?} \
            before initializing simulation",
            chunk = chunk,
            slabs = slab_count
        );

        world.request_slabs_with_count(slabs_to_request, slab_count);
        let timeout = Duration::from_secs(config::get().world.load_timeout as u64);
        world.block_for_last_batch_with_bail(timeout, panik::has_panicked)?;

        let mut sim = Simulation::new(world, resources)?;
        self.init(&mut sim, scenario)?;
        Ok((sim, initial_block))
    }
}

mod ci;
mod dev;
mod empty;

use crate::scenarios::Scenario;
pub use ci::ContinuousIntegrationGamePreset;
use config::WorldSource;
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
        config::WorldSource::Generate(file) => {
            debug!("generating world from config"; "path" => %file.display());

            let params = PlanetParams::load_with_only_file(resources, file.as_os_str());
            let source = params.and_then(|params| {
                pool.runtime()
                    .block_on(async { GeneratedTerrainSource::new(params).await })
            })?;
            WorldLoader::new(source, pool)
        }
    })
}
