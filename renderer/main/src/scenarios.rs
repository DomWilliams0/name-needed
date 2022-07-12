#![allow(dead_code)]

use common::*;
use engine::simulation;
use simulation::job::BuildThingJob;
use simulation::{ComponentWorld, EcsWorld, PlayerSociety, Scenario, Societies};

use crate::scenarios::helpers::{spawn_entities_randomly, Placement};

const DEFAULT_SCENARIO: &str = stringify!(wander_and_eat);

inventory::collect!(ScenarioEntry);

struct ScenarioEntry {
    /// Friendly name e.g. "Wander and eat"
    pub name: &'static str,
    /// Name for cmd line e.g. "wander_and_eat"
    pub id: &'static str,
    pub desc: &'static str,
    pub func: fn(&EcsWorld),
}

impl From<&ScenarioEntry> for Scenario {
    fn from(e: &ScenarioEntry) -> Self {
        Self {
            name: e.name,
            id: e.id,
            desc: e.desc,
            func: e.func,
        }
    }
}

pub fn resolve(id: Option<&str>) -> Option<Scenario> {
    let id = id.unwrap_or(DEFAULT_SCENARIO);
    iter().find(|e| e.id == id)
}

pub fn iter() -> impl Iterator<Item = Scenario> {
    inventory::iter::<ScenarioEntry>
        .into_iter()
        .map(Scenario::from)
}

macro_rules! scenario {
    ($func:path, $name:expr, $desc:expr) => {
        inventory::submit! { ScenarioEntry {name: $name, desc: $desc, func: $func, id: stringify!($func)} }
    };
}

// -------------

scenario!(
    wander_and_eat,
    "Wander and eat",
    "Spawn some people who wander around and pick up food"
);
scenario!(
    following_dogs,
    "Following dogs",
    "Spawn some dogs that follow people around"
);
scenario!(nop, "Empty", "Spawn nothing and do nothing");
scenario!(
    haul_to_container,
    "Haul to container",
    "Spawn some people to haul items into a container"
);
scenario!(
    building,
    "Wall building",
    "Spawn some people and bricks to build walls"
);
scenario!(
    herding,
    "Animal herding",
    "Spawn some animals that form into herds and wander around"
);

fn following_dogs(ecs: &EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let humans = helpers::get_config_count("humans");
    let dogs = helpers::get_config_count("dogs");

    let mut colors = helpers::entity_colours();

    let all_humans = spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next().unwrap())
            .with_player_society()
            .with_name()
            .thanks()
    });

    spawn_entities_randomly(&world, dogs, Placement::RandomPos, |pos| {
        let mut rand_althor = random::get();
        let human = all_humans
            .choose(&mut *rand_althor)
            .expect("no humans to follow");

        let dog = helpers::new_entity("core_living_dog", ecs, pos).thanks();
        ecs.helpers_dev().follow(dog, *human);

        dog
    });
}

pub fn nop(_: &EcsWorld) {}

fn wander_and_eat(ecs: &EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let mut colors = helpers::entity_colours();
    let humans = helpers::get_config_count("humans");
    let food = helpers::get_config_count("food");

    spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        let satiety = NormalizedFloat::new(random::get().gen_range(0.4, 0.5));
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next().unwrap())
            .with_player_society()
            .with_name()
            .with_satiety(satiety)
            .thanks()
    });

    spawn_entities_randomly(&world, food, Placement::RandomPos, |pos| {
        let nutrition = NormalizedFloat::new(random::get().gen_range(0.6, 1.0));
        let food = helpers::new_entity("core_food_apple", ecs, pos)
            .with_nutrition(nutrition)
            .thanks();

        food
    });
}

fn building(ecs: &EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let mut colors = helpers::entity_colours();
    let humans = helpers::get_config_count("humans");
    let food = helpers::get_config_count("food");

    let society = ecs
        .resource_mut::<PlayerSociety>()
        .get()
        .expect("no player society");

    let _humans = spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next().unwrap())
            .with_player_society()
            .with_satiety(NormalizedFloat::clamped(0.4))
            .with_name()
            .thanks()
    });

    let _food = spawn_entities_randomly(&world, food, Placement::RandomPos, |pos| {
        let nutrition = NormalizedFloat::new(random::get().gen_range(0.6, 1.0));
        let food = helpers::new_entity("core_food_apple", ecs, pos)
            .with_nutrition(nutrition)
            .thanks();

        food
    });

    let bricks = spawn_entities_randomly(
        &world,
        helpers::get_config_count("bricks"),
        Placement::RandomPos,
        |pos| helpers::new_entity("core_brick_stone", ecs, pos).thanks(),
    );

    let mut brick_stack = None;
    for brick in bricks {
        let res = match brick_stack {
            None => ecs
                .helpers_containers()
                .convert_to_stack(brick)
                .map(|stack| {
                    brick_stack = Some(stack);
                }),
            Some(stack) => ecs.helpers_containers().add_to_stack(stack, brick),
        };

        if let Err(err) = res {
            warn!("failed to stack bricks: {}", err);
            brick_stack = None;
        }
    }

    let society = ecs
        .resource_mut::<Societies>()
        .society_by_handle(society)
        .expect("bad society");

    if let Some(build) = ecs.find_build_template("core_build_wall") {
        let builds = helpers::get_config_count("build_jobs");
        for _ in 0..builds {
            let pos = helpers::random_walkable_pos(&world);

            society
                .jobs_mut()
                .submit(ecs, BuildThingJob::new(pos, build.clone()));
        }
    }
}

fn herding(ecs: &EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let mut colors = helpers::entity_colours();

    let _sheep = spawn_entities_randomly(
        &world,
        helpers::get_config_count("sheep"),
        Placement::RandomPos,
        |pos| {
            helpers::new_entity("core_living_sheep", ecs, pos)
                .with_satiety(NormalizedFloat::clamped(0.6))
                .thanks()
        },
    );

    spawn_entities_randomly(
        &world,
        helpers::get_config_count("cows"),
        Placement::RandomPos,
        |pos| {
            helpers::new_entity("core_living_cow", ecs, pos)
                .with_satiety(NormalizedFloat::clamped(0.6))
                .thanks()
        },
    );

    spawn_entities_randomly(
        &world,
        helpers::get_config_count("humans"),
        Placement::RandomPos,
        |pos| {
            helpers::new_entity("core_living_human", ecs, pos)
                .with_color(colors.next().unwrap())
                .with_player_society()
                .with_satiety(NormalizedFloat::new(0.2))
                .with_name()
                .thanks()
        },
    );
}

fn haul_to_container(ecs: &EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let mut colors = helpers::entity_colours();
    let humans = helpers::get_config_count("humans");
    let food = helpers::get_config_count("food");
    let bricks = helpers::get_config_count("bricks");

    let society = ecs
        .resource_mut::<PlayerSociety>()
        .get()
        .expect("no player society");

    // our lovely haulers
    spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next().unwrap())
            .with_player_society()
            .with_name()
            .thanks()
    });

    let food = spawn_entities_randomly(&world, food, Placement::RandomPos, |pos| {
        helpers::new_entity("core_food_apple", ecs, pos).thanks()
    });

    let bricks = spawn_entities_randomly(&world, bricks, Placement::RandomPos, |pos| {
        helpers::new_entity("core_brick_stone", ecs, pos).thanks()
    });

    let chest_pos = helpers::create_chest(ecs, &world, None);
    food.iter().chain(bricks.iter()).for_each(|e| {
        ecs.helpers_dev()
            .haul_to_container_via_society(society, *e, chest_pos);
    });
}

mod helpers {
    use color::Color;
    use common::{random, NormalizedFloat, Rng};
    use engine::simulation;
    use engine::simulation::NameGeneration;
    use simulation::{
        BlockType, ComponentWorld, ConditionComponent, EcsWorld, Entity, EntityLoggingComponent,
        EntityPosition, HungerComponent, InnerWorldRef, PlayerSociety, RenderComponent,
        SocietyComponent, SocietyHandle, WorldPosition, WorldPositionRange, WorldTerrainUpdate,
    };

    use crate::simulation::{NameComponent, TerrainUpdatesRes};

    pub fn get_config_count(wat: &str) -> usize {
        let counts = &config::get().simulation.spawn_counts;
        counts.get(wat).copied().unwrap_or(0)
    }

    pub fn entity_colours() -> impl Iterator<Item = Color> {
        Color::unique_randoms(
            NormalizedFloat::new(0.65),
            NormalizedFloat::new(0.4),
            &mut *random::get(),
        )
    }

    pub struct EntityBuilder<'a>(&'a EcsWorld, Entity);

    pub enum Placement {
        RandomPos,
        RandomPosAndRot,
        // TODO random pos offset away from the voxel centre
    }

    impl<'a> EntityBuilder<'a> {
        fn new(definition: &str, pos: impl EntityPosition + 'static, world: &'a EcsWorld) -> Self {
            let entity = world
                .build_entity(definition)
                .expect("no definition")
                .with_position(pos)
                .spawn()
                .expect("failed to create entity");

            Self(world, entity)
        }

        pub fn with_color(self, color: Color) -> Self {
            self.0
                .component_mut::<RenderComponent>(self.1)
                .map(|mut render| render.color = color)
                .expect("render component");

            self
        }

        pub fn with_society(self, society: SocietyHandle) -> Self {
            self.0
                .add_now(self.1, SocietyComponent::new(society))
                .expect("society component");

            self
        }

        pub fn with_player_society(self) -> Self {
            let player_society = self
                .0
                .resource::<PlayerSociety>()
                .get()
                .expect("no player society");
            self.with_society(player_society)
        }

        pub fn with_satiety(self, satiety: NormalizedFloat) -> Self {
            self.0
                .component_mut::<HungerComponent>(self.1)
                .map(|mut hunger| hunger.hunger_mut().set_satiety(satiety))
                .expect("hunger component");

            self
        }

        pub fn with_name(self) -> Self {
            let mut rng = random::get();
            let name = self.0.resource::<NameGeneration>().generate(&mut *rng);
            let _ = self.0.add_now(self.1, NameComponent::new(name.to_owned()));
            self
        }

        pub fn with_condition(self, condition: NormalizedFloat) -> Self {
            self.0
                .component_mut::<ConditionComponent>(self.1)
                .map(|mut comp| comp.0.set(condition))
                .expect("condition component");

            self
        }

        pub fn with_nutrition(self, nutrition: NormalizedFloat) -> Self {
            self.with_condition(nutrition)
        }

        pub fn thanks(self) -> Entity {
            // add logging to all entities if configured
            if config::get().simulation.entity_logging_by_default {
                let _ = self.0.add_now(self.1, EntityLoggingComponent::default());
            }

            self.1
        }
    }

    pub fn new_entity<'a>(
        definition: &str,
        ecs: &'a EcsWorld,
        pos: impl EntityPosition + 'static,
    ) -> EntityBuilder<'a> {
        EntityBuilder::new(definition, pos, ecs)
    }

    pub fn random_walkable_pos(world: &InnerWorldRef) -> WorldPosition {
        world
            .choose_random_walkable_block(500)
            .expect("failed to find a random walkable position")
    }

    pub fn spawn_entities_randomly(
        world: &InnerWorldRef,
        count: usize,
        placement: Placement,
        mut per: impl FnMut((WorldPosition, f32)) -> Entity,
    ) -> Vec<Entity> {
        (0..count)
            .map(|_| {
                let pos = random_walkable_pos(world);
                let rot = match placement {
                    Placement::RandomPos => 0.0,
                    Placement::RandomPosAndRot => random::get().gen_range(0.0f32, 360.0),
                };
                per((pos, rot))
            })
            .collect()
    }

    pub fn create_chest(
        ecs: &EcsWorld,
        world: &InnerWorldRef,
        society: Option<SocietyHandle>,
    ) -> WorldPosition {
        let chest_pos = random_walkable_pos(world);

        let terrain_updates = ecs.resource_mut::<TerrainUpdatesRes>();
        terrain_updates.push(WorldTerrainUpdate::new(
            WorldPositionRange::with_single(chest_pos),
            BlockType::Chest,
        ));

        if society.is_some() {
            ecs.helpers_dev()
                .make_container_communal(chest_pos, society);
        }

        chest_pos
    }
}
