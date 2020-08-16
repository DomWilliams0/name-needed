use std::path::Path;

use common::*;
use simulation::{
    presets, BaseItemComponent, ComponentWorld, GeneratedTerrainSource, HungerComponent,
    RenderComponent, Renderer, Simulation, SocietyComponent, ThreadedWorkerPool,
    ThreadedWorldLoader, WorldLoader,
};

use crate::GamePreset;
use color::ColorRgb;
use config::WorldSource;
use simulation::dev::SimulationDevExt;

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

    fn world(&self) -> BoxedResult<ThreadedWorldLoader> {
        let thread_count = config::get()
            .world
            .worker_threads
            .unwrap_or_else(|| (num_cpus::get() / 2).max(1));
        my_debug!(
            "using {threads} threads for world loader",
            threads = thread_count
        );
        let pool = ThreadedWorkerPool::new(thread_count);

        let which_source = config::get().world.source.clone();
        Ok(match which_source {
            WorldSource::Preset(preset) => {
                my_debug!("loading world preset"; "preset" => ?preset);
                let source = presets::from_preset(preset);
                WorldLoader::new(source, pool)
            }
            WorldSource::Generate { seed, radius } => {
                my_debug!("generating world with radius {radius}", radius = radius);
                let height_scale = config::get().world.generation_height_scale;
                let source = GeneratedTerrainSource::new(seed, radius, height_scale)?;
                WorldLoader::new(source, pool)
            }
        })
    }

    fn init(&self, sim: &mut Simulation<R>) -> BoxedResult<()> {
        if let Some(seed) = config::get().simulation.random_seed {
            random::reseed(seed);
            my_info!(
                "seeding random generator with seed {seed} from config",
                seed = seed
            );
        } else {
            my_info!("using random seed")
        }

        let mut colors = ColorRgb::unique_randoms(0.65, 0.4, &mut *random::get()).unwrap();

        // add random entities
        let randoms = config::get().simulation.random_count;
        let worldref = sim.world();
        let world = worldref.borrow();
        if randoms > 0 {
            my_info!("adding {count} random entities", count = randoms);

            let society = sim
                .societies()
                .new_society("Nice People".to_owned())
                .unwrap();
            *sim.player_society() = Some(society);

            for _ in 0..randoms {
                let (pos, satiety) = {
                    let pos = world
                        .choose_random_walkable_block(50)
                        .expect("random entity placement failed");
                    let satiety = NormalizedFloat::new(random::get().gen_range(0.4, 0.5));
                    (pos, satiety)
                };
                let color = colors.next().unwrap(); // infinite iterator

                let human = sim
                    .entity_builder("core_living_human")
                    .expect("no human")
                    .with_position(pos)
                    .spawn()
                    .expect("failed to create human");

                // customize
                let ecs = sim.world_mut();
                ecs.add_now(human, SocietyComponent::new(society))
                    .expect("society component");

                ecs.component_mut::<RenderComponent>(human)
                    .map(|render| render.color = color)
                    .expect("render component");

                ecs.component_mut::<HungerComponent>(human)
                    .map(|hunger| hunger.set_satiety(satiety))
                    .expect("hunger component");
            }

            // add some random food items too
            let food_count = config::get().simulation.food_count;
            for _ in 0..food_count {
                let nutrition = NormalizedFloat::new(random::get().gen_range(0.6, 1.0));
                if let Some(pos) = world.choose_random_walkable_block(20) {
                    let food = sim
                        .entity_builder("core_food_apple")
                        .expect("no apple")
                        .with_position(pos)
                        .spawn()
                        .expect("food");

                    sim.world_mut()
                        .component_mut::<BaseItemComponent>(food)
                        .map(|item| item.condition.set(nutrition))
                        .expect("nutrition");
                }
            }
        }

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
