use std::path::Path;

use common::*;
use simulation::{
    presets, ComponentWorld, InventoryComponent, PhysicalShape, Renderer, Simulation,
    ThreadedWorkerPool, ThreadedWorldLoader, WorldLoader,
};

use crate::GamePreset;
use color::ColorRgb;
use simulation::dev::SimulationDevExt;
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
        if let Some(seed) = config::get().simulation.random_seed {
            random::reseed(seed);
            debug!("seeding random generator with seed {:?} from config", seed);
        }

        let mut colors = ColorRgb::unique_randoms(0.65, 0.4).unwrap();

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
            let human = (0..randoms)
                .map(|i| {
                    let (pos, radius) = {
                        let mut randy = random::get();
                        let pos = (randy.gen_range(0, 5), randy.gen_range(0, 5), None);
                        let radius = randy.gen_range(0.3, 0.5);
                        (pos, radius)
                    };
                    let color = colors.next().unwrap();

                    trace!(
                        "entity {}: pos {:?}, radius: {:?}, color: {:?}",
                        i,
                        pos,
                        radius,
                        color
                    );

                    match sim
                        .add_entity()
                        .with_pos(pos)
                        .with_height(1.0)
                        .with_shape(PhysicalShape::circle(radius))
                        .with_color(color)
                        .build_human(NormalizedFloat::new(random::get().gen_range(0.4, 0.5)))
                    {
                        Err(e) => {
                            warn!("failed to create random human: {}", e);
                            None
                        }
                        Ok(e) => {
                            debug!("creating human {:?}", e);
                            Some(e)
                        }
                    }
                })
                .last()
                .unwrap()
                .expect("surely one human succeeded");

            // add some random food items too
            let worldref = sim.world();
            let world = worldref.borrow();
            let food_count = config::get().simulation.food_count;
            for _ in 0..food_count {
                let nutrition = config::get().simulation.food_nutrition;
                if let Some(pos) = world.choose_random_walkable_block(20) {
                    match sim
                        .add_entity()
                        .with_pos(pos)
                        .with_height(0.5)
                        .with_shape(PhysicalShape::square(0.15))
                        .with_color(ColorRgb::new_hsl(
                            0.3,
                            0.64,
                            random::get().gen_range(0.4, 0.9),
                        ))
                        .build_food_item(nutrition)
                    {
                        Err(e) => {
                            warn!("failed to create random food entity: {}", e);
                        }
                        Ok(item) => {
                            // if i == 0 {
                            //     sim.make_food_bag_and_give_to(item, human);
                            // }
                        }
                    }
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
