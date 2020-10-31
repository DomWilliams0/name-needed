use std::path::Path;

use color::ColorRgb;
use common::*;
use config::WorldSource;
use simulation::{
    presets, BlockType, ComponentWorld, ConditionComponent, GeneratedTerrainSource,
    HungerComponent, RenderComponent, Renderer, Simulation, SocietyComponent, TerrainUpdatesRes,
    ThreadedWorkerPool, ThreadedWorldLoader, TransformComponent, WorldLoader, WorldPositionRange,
    WorldTerrainUpdate,
};

use crate::GamePreset;

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

        let mut colors = ColorRgb::unique_randoms(0.65, 0.4, &mut *random::get()).unwrap();

        // add random entities
        let entity_count = |name| {
            let counts = &config::get().simulation.spawn_counts;
            counts.get(name).copied().unwrap_or(0)
        };

        let humans = entity_count("humans");
        let dogs = entity_count("dogs");

        let worldref = sim.voxel_world();
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

        let ecs = sim.world_mut();

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

                    let human = ecs
                        .build_entity("core_living_human")
                        .expect("no human")
                        .with_position(pos)
                        .spawn()
                        .expect("failed to create human");

                    // customize
                    ecs.add_now(human, SocietyComponent::new(society))
                        .expect("society component");

                    ecs.component_mut::<RenderComponent>(human)
                        .map(|render| render.color = color)
                        .expect("render component");

                    ecs.helpers_dev().give_bag(human);

                    all_humans.push(human);
                    human
                }
                DoggoOrNot::ThisIsDog => {
                    let dog = ecs
                        .build_entity("core_living_dog")
                        .expect("no dog")
                        .with_position(pos)
                        .spawn()
                        .expect("failed to create dog");

                    all_dogs.push(dog);
                    dog
                }
            };

            ecs.component_mut::<HungerComponent>(entity)
                .map(|hunger| hunger.set_satiety(satiety))
                .expect("hunger component");
        }

        // some dogs follow humans
        {
            let mut randy = random::get();
            for dog in all_dogs {
                let human = all_humans.choose(&mut *randy).unwrap();
                ecs.helpers_dev().follow(dog, *human);
            }
        }

        // add some random food items too
        let food_count = entity_count("food");
        let mut all_food = Vec::new();
        for _ in 0..food_count {
            let nutrition = NormalizedFloat::new(random::get().gen_range(0.6, 1.0));
            if let Some(pos) = world.choose_random_walkable_block(20) {
                let food = ecs
                    .build_entity("core_food_apple")
                    .expect("no apple")
                    .with_position(pos)
                    .spawn()
                    .expect("food");

                ecs.component_mut::<ConditionComponent>(food)
                    .map(|condition| condition.0.set(nutrition))
                    .expect("nutrition");

                all_food.push(food);
            }
        }

        // and random bricks
        for _ in 0..entity_count("bricks") {
            if let Some(pos) = world.choose_random_walkable_block(20) {
                let brick = ecs
                    .build_entity("core_brick_stone")
                    .expect("no brick")
                    .with_position(pos)
                    .spawn()
                    .expect("brick");

                let angle = random::get().gen_range(0.0, 3.15 /* sue me */);
                ecs.component_mut::<TransformComponent>(brick)
                    .unwrap()
                    .rotate_to(rad(angle));
            }
        }

        // give a load of free food
        let drain_count = 3;
        for (food, human) in all_food
            .drain(..drain_count.min(all_food.len()))
            .zip(all_humans.first().into_iter().cycle())
        {
            ecs.helpers_dev().put_food_in_container(food, *human);
            // sim.eat(*human, *food);
        }

        // place a chest
        let chest_pos = world
            .choose_random_walkable_block(100)
            .expect("cant face place for chest");

        let terrain_updates = ecs.resource_mut::<TerrainUpdatesRes>();
        terrain_updates.push(WorldTerrainUpdate::new(
            WorldPositionRange::with_single(chest_pos),
            BlockType::Chest,
        ));

        // haul a food to the chest
        if let Some((food, human)) = all_food.first().zip(all_humans.first()) {
            ecs.helpers_dev()
                .haul_to_container(*human, *food, chest_pos);
        }

        ecs.helpers_dev()
            .make_container_communal(chest_pos, Some(society));

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
