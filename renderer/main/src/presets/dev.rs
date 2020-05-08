use std::path::Path;

use common::*;
use simulation::{
    presets, Renderer, Simulation, ThreadedWorkerPool, ThreadedWorldLoader, WorldLoader,
};

use crate::GamePreset;
use color::ColorRgb;
use std::marker::PhantomData;

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

    fn world(&self) -> ThreadedWorldLoader {
        let thread_count = config::get()
            .world
            .worker_threads
            .unwrap_or_else(|| (num_cpus::get() / 2).max(1));
        debug!("using {} threads for world loader", thread_count);
        let pool = ThreadedWorkerPool::new(thread_count);
        WorldLoader::new(presets::from_config(), pool)
    }

    fn init(&self, sim: &mut Simulation<R>) {
        let mut rng = if let Some(seed) = config::get().simulation.random_seed {
            debug!("using rng seed {} from config", seed);
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_entropy()
        };

        let mut colors = ColorRgb::unique_randoms(&mut rng, 0.85, 0.4).unwrap();

        // add entities from config
        /*{
                    let dummies = config::get().simulation.initial_entities.clone();
                    info!("adding {} dummy entities", dummies.len());
                    for desc in dummies {
                        let color = desc
                            .color
                            .map(ColorRgb::from)
                            .unwrap_or_else(|| colors.next().unwrap());
                        // sim.add_entity(desc.pos, color, desc.size);
                    }
                }
        */
        // add random entities
        let randoms = config::get().simulation.random_count;
        if randoms > 0 {
            info!("adding {} random entities", randoms);
            for i in 0..randoms {
                let pos = (4 + rng.gen_range(-4, 4), 4 + rng.gen_range(-4, 4), None);
                let color = colors.next().unwrap();
                let diameter = rng.gen_range(0.5, 0.9);

                trace!(
                    "entity {}: pos {:?}, radius: {:?}, color: {:?}",
                    i,
                    pos,
                    diameter,
                    color
                );

                match sim
                    .add_entity()
                    .with_transform(pos)
                    .with_physical(diameter, color)
                    .with_wandering_human_archetype()
                    .build()
                {
                    Err(e) => warn!("failed to create random entity: {}", e),
                    Ok(e) => debug!("creating entity {:?}", e),
                }
            }
        }
    }
}

impl<R: Renderer> Default for DevGamePreset<R> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
