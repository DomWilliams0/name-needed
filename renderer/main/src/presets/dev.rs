use std::path::Path;

use common::*;

use crate::presets::world_from_source;
use crate::scenarios::Scenario;
use crate::GamePreset;
use simulation::{AsyncWorkerPool, Renderer, Simulation, ThreadedWorldLoader};

pub struct DevGamePreset<R: Renderer> {
    _phantom: PhantomData<R>,
}

impl<R: Renderer> GamePreset<R> for DevGamePreset<R> {
    fn name(&self) -> &str {
        "dev"
    }

    fn config(&self) -> Option<&Path> {
        Some(Path::new("config.ron"))
    }

    fn world(&self, resources: &resources::WorldGen) -> BoxedResult<ThreadedWorldLoader> {
        let thread_count = config::get()
            .world
            .worker_threads
            .unwrap_or_else(|| (num_cpus::get() / 2).max(1));
        debug!(
            "using {threads} threads for world loader",
            threads = thread_count
        );
        let pool = AsyncWorkerPool::new(thread_count)?;

        let which_source = config::get().world.source.clone();
        world_from_source(which_source, pool, resources)
    }

    fn init(&self, sim: &mut Simulation<R>, scenario: Scenario) -> BoxedResult<()> {
        let (seed, source) = if let Some(seed) = config::get().simulation.random_seed {
            (seed, "config")
        } else {
            (thread_rng().next_u64(), "randomly generated")
        };

        random::reseed(seed);
        info!(
            "seeding random generator with seed {seed}",
            seed = seed; "source" => source
        );

        // create society for player to control
        let player_society = sim
            .societies()
            .new_society("Top Geezers".to_owned())
            .unwrap();
        *sim.player_society() = Some(player_society);

        // defer to scenario for all entity spawning
        scenario(sim.world_mut());
        Ok(())
    }
}

impl<R: Renderer> Default for DevGamePreset<R> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
