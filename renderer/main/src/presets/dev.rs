use std::path::Path;

use common::*;
use simulation::{Renderer, Simulation};

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

    fn init(&self, sim: &mut Simulation<R>) {
        let mut rng = thread_rng();
        let mut colors = ColorRgb::unique_randoms(&mut rng, 0.85, 0.4).unwrap();

        // add entities from config
        {
            let dummies = config::get().simulation.initial_entities.clone();
            info!("adding {} dummy entities", dummies.len());
            for desc in dummies {
                let color = desc
                    .color
                    .map(ColorRgb::from)
                    .unwrap_or_else(|| colors.next().unwrap());
                sim.add_entity(desc.pos, color, desc.size);
            }
        }

        // add random entities
        let randoms = config::get().simulation.random_count;
        if randoms > 0 {
            info!("adding {} random entities", randoms);
            for _ in 0..randoms {
                let pos = (4 + rng.gen_range(-4, 4), 4 + rng.gen_range(-4, 4), Some(3));
                let color = colors.next().unwrap();
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

impl<R: Renderer> Default for DevGamePreset<R> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
