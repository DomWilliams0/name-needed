use std::path::Path;

use common::*;
use simulation::{
    presets, GeneratedTerrainSource, PhysicalShape, Renderer, Simulation, ThreadedWorkerPool,
    ThreadedWorldLoader, WorldLoader,
};

use crate::GamePreset;
use color::ColorRgb;
use config::WorldSource;

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

        let which_source = config::get().world.source.clone();
        match which_source {
            WorldSource::Preset(preset) => {
                debug!("loading world preset '{:?}'", preset);
                let source = presets::from_preset(preset);
                WorldLoader::new(source, pool)
            }
            WorldSource::Generate { seed, radius } => {
                debug!("generating world with radius {}", radius);
                let height_scale = config::get().world.generation_height_scale;
                match GeneratedTerrainSource::new(seed, radius, height_scale) {
                    // TODO GamePreset::world() should return a Result
                    Err(e) => panic!("bad params for world generation: {}", e),
                    Ok(source) => WorldLoader::new(source, pool),
                }
            }
        }
    }

    fn init(&self, sim: &mut Simulation<R>) {
        if let Some(seed) = config::get().simulation.random_seed {
            random::reseed(seed);
            debug!("seeding random generator with seed {:?} from config", seed);
        }

        let mut colors = ColorRgb::unique_randoms(0.65, 0.4, &mut *random::get()).unwrap();

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
        let worldref = sim.world();
        let world = worldref.borrow();
        if randoms > 0 {
            info!("adding {} random entities", randoms);

            let society = sim
                .societies()
                .new_society("Nice People".to_owned())
                .unwrap();
            *sim.player_society() = Some(society);

            let _human = (0..randoms)
                .map(|i| {
                    let (pos, radius) = {
                        let pos = world
                            .choose_random_walkable_block(50)
                            .expect("random entity placement failed");
                        let radius = random::get().gen_range(0.3, 0.5);
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
                        .with_society(society)
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
            let food_count = config::get().simulation.food_count;
            for _ in 0..food_count {
                let nutrition = config::get().simulation.food_nutrition;
                if let Some(pos) = world.choose_random_walkable_block(20) {
                    let mut randy = random::get();
                    match sim
                        .add_entity()
                        .with_pos(pos)
                        .with_height(0.5)
                        .with_shape(PhysicalShape::square(0.15))
                        .with_color(ColorRgb::new_hsl(0.3, 0.64, randy.gen_range(0.4, 0.9)))
                        .build_food_item(
                            ((nutrition as f32) * randy.gen_range(0.8, 1.2)) as u16,
                            randy.gen_range(0.2, 1.0),
                        ) {
                        Err(e) => {
                            warn!("failed to create random food entity: {}", e);
                        }
                        Ok(_item) => {
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
