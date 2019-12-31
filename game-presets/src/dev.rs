use std::path::Path;

use common::*;
use simulation::{Renderer, Simulation};

use crate::GamePreset;
use world::WorldRef;

pub struct DevGamePreset;

impl GamePreset for DevGamePreset {
    fn name(&self) -> &str {
        "dev"
    }

    fn config(&self) -> Option<&Path> {
        Some(Path::new("config.ron"))
    }

    fn world(&self) -> WorldRef {
        WorldRef::new(world::presets::from_config())
    }

    fn init<R: Renderer>(&self, sim: &mut Simulation<R>) {
        // add entities from config
        {
            let dummies = config::get().simulation.initial_entities.clone();
            info!("adding {} dummy entities", dummies.len());
            for desc in dummies {
                sim.add_entity(desc.pos, desc.color, desc.size);
            }
        }

        // add random entities
        let randoms = config::get().simulation.random_count;
        if randoms > 0 {
            info!("adding {} random entities", randoms);
            let mut rng = thread_rng();
            for _ in 0..randoms {
                let pos = (4 + rng.gen_range(-4, 4), 4 + rng.gen_range(-4, 4), Some(3));
                let color = (
                    rng.gen_range(20, 230u8),
                    rng.gen_range(20, 230u8),
                    rng.gen_range(20, 230u8),
                );
                let dims = (
                    rng.gen_range(0.8, 1.1),
                    rng.gen_range(0.9, 1.1),
                    rng.gen_range(1.4, 2.0),
                );

                sim.add_entity(pos, color, dims);
            }
        }
    }
}
