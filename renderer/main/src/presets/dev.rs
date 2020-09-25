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
        debug!(
            "using {threads} threads for world loader",
            threads = thread_count
        );
        let pool = ThreadedWorkerPool::new(thread_count);

        let which_source = config::get().world.source.clone();
        Ok(match which_source {
            WorldSource::Preset(preset) => {
                debug!("loading world preset"; "preset" => ?preset);
                let source = presets::from_preset(preset);
                WorldLoader::new(source, pool)
            }
            WorldSource::Generate { seed, radius } => {
                debug!("generating world with radius {radius}", radius = radius);
                let height_scale = config::get().world.generation_height_scale;
                let source = GeneratedTerrainSource::new(seed, radius, height_scale)?;
                WorldLoader::new(source, pool)
            }
        })
    }

    fn init(&self, sim: &mut Simulation<R>) -> BoxedResult<()> {
        if let Some(seed) = config::get().simulation.random_seed {
            random::reseed(seed);
            info!(
                "seeding random generator with seed {seed} from config",
                seed = seed
            );
        } else {
            info!("using random seed")
        }

        let mut colors = ColorRgb::unique_randoms(0.65, 0.4, &mut *random::get()).unwrap();

        // add random entities
        let (humans, dogs) = {
            let conf = &config::get().simulation;
            (conf.human_count, conf.dog_count)
        };

        let worldref = sim.world();
        let world = worldref.borrow();
        info!(
            "adding {humans} humans and {dogs} dogs",
            humans = humans,
            dogs = dogs
        );

        let society = sim
            .societies()
            .new_society("Nice People".to_owned())
            .unwrap();
        *sim.player_society() = Some(society);

        // gross but temporary
        #[derive(Clone)]
        enum DoggoOrNot {
            Human,
            ThisIsDog,
        }

        let randoms =
            repeat_n(DoggoOrNot::Human, humans).chain(repeat_n(DoggoOrNot::ThisIsDog, dogs));

        let mut all_humans = Vec::with_capacity(humans);
        let mut all_dogs = Vec::with_capacity(dogs);
        for rando in randoms {
            let (pos, satiety) = {
                let pos = world
                    .choose_random_walkable_block(50)
                    .expect("random entity placement failed");
                let satiety = NormalizedFloat::new(random::get().gen_range(0.4, 0.5));
                (pos, satiety)
            };

            let entity = match rando {
                DoggoOrNot::Human => {
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

                    all_humans.push(human);
                    human
                }
                DoggoOrNot::ThisIsDog => {
                    let dog = sim
                        .entity_builder("core_living_dog")
                        .expect("no dog")
                        .with_position(pos)
                        .spawn()
                        .expect("failed to create dog");

                    all_dogs.push(dog);
                    dog
                }
            };

            sim.world_mut()
                .component_mut::<HungerComponent>(entity)
                .map(|hunger| hunger.set_satiety(satiety))
                .expect("hunger component");
        }

        // some dogs follow humans
        {
            let mut randy = random::get();
            for dog in all_dogs {
                let human = all_humans.choose(&mut *randy).unwrap();
                sim.follow(dog, *human);
            }
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
