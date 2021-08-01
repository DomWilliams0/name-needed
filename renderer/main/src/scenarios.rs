#![allow(dead_code)]
use crate::scenarios::helpers::{spawn_entities_randomly, Placement};
use common::*;
use simulation::{ActivityComponent, ComponentWorld, EcsWorld, PlayerSociety, Societies};

pub type Scenario = fn(&mut EcsWorld);
const DEFAULT_SCENARIO: &str = "wander_and_eat";

struct ScenarioEntry {
    pub name: &'static str,
    pub func: Scenario,
}

inventory::collect!(ScenarioEntry);

pub fn resolve(name: Option<&str>) -> Option<(&str, Scenario)> {
    match name {
        None => {
            let default = resolve(Some(DEFAULT_SCENARIO)).expect("bad default");
            Some(default)
        }
        Some(name) => inventory::iter::<ScenarioEntry>
            .into_iter()
            .find(|e| e.name == name)
            .map(|e| (e.name, e.func)),
    }
}

pub fn all_names() -> impl Iterator<Item = &'static str> {
    inventory::iter::<ScenarioEntry>.into_iter().map(|e| e.name)
}

macro_rules! scenario {
    ($name:expr, $func:path) => {
        inventory::submit! { ScenarioEntry {name: $name, func: $func}, }
    };

    ($func:path) => {
        inventory::submit! { ScenarioEntry {name: stringify!($func), func: $func} }
    };
}

// -------------

scenario!(following_dogs);
scenario!(nop);
scenario!(wander_and_eat);
scenario!(haul_to_container);
scenario!(log_cutting);

fn following_dogs(ecs: &mut EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let humans = helpers::get_config_count("humans");
    let dogs = helpers::get_config_count("dogs");

    let mut colors = helpers::entity_colours();

    let all_humans = spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next_please())
            .with_player_society()
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

fn nop(_: &mut EcsWorld) {}

fn wander_and_eat(ecs: &mut EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let mut colors = helpers::entity_colours();
    let humans = helpers::get_config_count("humans");
    let food = helpers::get_config_count("food");

    spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        let satiety = NormalizedFloat::new(random::get().gen_range(0.4, 0.5));
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next_please())
            .with_player_society()
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

fn log_cutting(ecs: &mut EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let mut colors = helpers::entity_colours();
    let humans = helpers::get_config_count("humans");

    let society = ecs
        .resource_mut::<PlayerSociety>()
        .0
        .expect("no player society");

    let humans = spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next_please())
            .with_player_society()
            .thanks()
    });

    let trunk = spawn_entities_randomly(&world, 1, Placement::RandomPosAndRot, |pos| {
        helpers::new_entity("core_tree_trunk", ecs, pos)
            .with_condition(NormalizedFloat::one())
            .thanks()
    })[0];

    let saw = spawn_entities_randomly(&world, 1, Placement::RandomPosAndRot, |pos| {
        helpers::new_entity("core_saw", ecs, pos)
            .with_condition(NormalizedFloat::one())
            .thanks()
    })[0];

    if let Some(human) = humans.first().copied() {
        let society = ecs
            .resource_mut::<Societies>()
            .society_by_handle(society)
            .expect("bad society");
        let activity = ecs
            .component_mut::<ActivityComponent>(human)
            .expect("no activity");

        // TODO
        // let location = {
        //     let pos = ecs
        //         .component::<TransformComponent>(trunk)
        //         .expect("no trunk transform")
        //         .position;
        //     // TODO work item should encompass full trunk
        //     let point = geo::Point::new(pos.x(), pos.y());
        //     Location::new(point, pos.z())
        // };
        // let work_item = society
        //     .work_items_mut()
        //     .add(WorkItem::new(location, TreeLogCuttingWorkItem::default()));
        //
        // activity.interrupt_with_new_activity(
        //     AiAction::GoWorkOnWorkItem(work_item),
        //     None,
        //     human,
        //     ecs,
        // );
    }
}

fn haul_to_container(ecs: &mut EcsWorld) {
    let world = ecs.voxel_world();
    let world = world.borrow();

    let mut colors = helpers::entity_colours();
    let humans = helpers::get_config_count("humans");
    let food = helpers::get_config_count("food");
    let bricks = helpers::get_config_count("bricks");

    let society = ecs
        .resource_mut::<PlayerSociety>()
        .0
        .expect("no player society");

    // our lovely haulers
    spawn_entities_randomly(&world, humans, Placement::RandomPos, |pos| {
        helpers::new_entity("core_living_human", ecs, pos)
            .with_color(colors.next_please())
            .with_player_society()
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
    use color::{ColorRgb, UniqueRandomColors};
    use common::{random, NormalizedFloat, Rng};
    use simulation::{
        BlockType, ComponentWorld, ConditionComponent, EcsWorld, Entity, EntityLoggingComponent,
        EntityPosition, HungerComponent, InnerWorldRef, PlayerSociety, RenderComponent,
        SocietyComponent, SocietyHandle, TerrainUpdatesRes, WorldPosition, WorldPositionRange,
        WorldTerrainUpdate,
    };

    pub fn get_config_count(wat: &str) -> usize {
        let counts = &config::get().simulation.spawn_counts;
        counts.get(wat).copied().unwrap_or(0)
    }

    pub fn entity_colours() -> UniqueRandomColors {
        ColorRgb::unique_randoms(0.65, 0.4, &mut *random::get()).unwrap()
    }

    pub struct EntityBuilder<'a>(&'a mut EcsWorld, Entity);

    pub enum Placement {
        RandomPos,
        RandomPosAndRot,
        // TODO random pos offset away from the voxel centre
    }

    impl<'a> EntityBuilder<'a> {
        fn new(
            definition: &str,
            pos: impl EntityPosition + 'static,
            world: &'a mut EcsWorld,
        ) -> Self {
            let entity = world
                .build_entity(definition)
                .expect("no definition")
                .with_position(pos)
                .spawn()
                .expect("failed to create entity");

            Self(world, entity)
        }

        pub fn with_color(self, color: ColorRgb) -> Self {
            self.0
                .component_mut::<RenderComponent>(self.1)
                .map(|render| render.color = color)
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
                .0
                .expect("no player society");
            self.with_society(player_society)
        }

        pub fn with_satiety(self, satiety: NormalizedFloat) -> Self {
            self.0
                .component_mut::<HungerComponent>(self.1)
                .map(|hunger| hunger.set_satiety(satiety))
                .expect("hunger component");

            self
        }

        pub fn with_condition(self, condition: NormalizedFloat) -> Self {
            self.0
                .component_mut::<ConditionComponent>(self.1)
                .map(|comp| comp.0.set(condition))
                .expect("condition component");

            self
        }

        pub fn with_nutrition(self, nutrition: NormalizedFloat) -> Self {
            self.with_condition(nutrition)
        }

        pub fn with_logging(self) -> Self {
            self.0
                .add_now(self.1, EntityLoggingComponent::default())
                .expect("logging component");
            self
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
        ecs: &'a mut EcsWorld,
        pos: impl EntityPosition + 'static,
    ) -> EntityBuilder<'a> {
        EntityBuilder::new(definition, pos, ecs)
    }

    pub fn random_walkable_pos(world: &InnerWorldRef) -> WorldPosition {
        world
            .choose_random_walkable_block(50)
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
        ecs: &mut EcsWorld,
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
