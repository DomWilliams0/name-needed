use std::cmp::Ordering;
use std::num::NonZeroU16;
use std::rc::Rc;

use common::*;
use simulation::job::BuildThingJob;
use simulation::{
    BlockType, BuildMaterial, BuildTemplate, CachedStr, ComponentWorld, ContainersError, Entity,
    EntityEventDebugPayload, EntityEventPayload, ItemStackError, Societies, SocietyComponent,
    StackableComponent, TaskResultSummary,
};
use unit::world::WorldPosition;

use crate::helpers::EntityPosition;
use crate::tests::TestHelper;
use crate::{HookContext, HookResult, InitHookResult, TestDeclaration};

pub struct GatherAndBuild {
    builder: Entity,
    remaining_builds: usize,
    first_time_finishing: bool,
    all_builds: Vec<WorldPosition>,
    all_bricks: Vec<Entity>,
}

const BRICKS_PER_WALL: u16 = 6;
const MAX_STACKABILITY: u16 = 8;

impl GatherAndBuild {
    pub fn on_tick(&mut self, test: TestHelper, ctx: &HookContext) -> HookResult {
        let finished_builds = ctx
            .events
            .iter()
            .filter(|evt| {
                if evt.subject != self.builder {
                    return false;
                };
                match &evt.payload {
                    EntityEventPayload::Debug(EntityEventDebugPayload::FinishedActivity {
                        description,
                        result,
                    }) if description.contains("Building") => match result {
                        TaskResultSummary::Cancelled => false,
                        TaskResultSummary::Succeeded => true,
                        TaskResultSummary::Failed(err) => panic!("build activity failed: {}", err),
                    },
                    _ => false,
                }
            })
            .count();

        match finished_builds.cmp(&self.remaining_builds) {
            Ordering::Less => return HookResult::KeepGoing,
            Ordering::Equal => {}
            Ordering::Greater => panic!("too many finished builds??"),
        }

        if self.first_time_finishing {
            self.first_time_finishing = false;
            test.wait_n_ticks(20);
            return HookResult::KeepGoing;
        }

        // check builds actually succeeded
        let w = ctx.simulation.world;
        let w = w.borrow();
        for pos in self.all_builds.iter() {
            let block = w.block(*pos).expect("bad pos");
            assert_eq!(
                block.block_type(),
                BlockType::StoneBrickWall,
                "expected built wall at {}",
                pos
            );
        }

        let bad_bricks = self
            .all_bricks
            .iter()
            .copied()
            .filter(|e| ctx.simulation.ecs.is_entity_alive(*e))
            .collect_vec();
        assert!(
            bad_bricks.is_empty(),
            "expected all bricks to be consumed by now but these are still alive: {:?}",
            bad_bricks
        );

        HookResult::TestSuccess
    }

    pub fn on_init(test: TestHelper, ctx: &HookContext) -> InitHookResult<Self> {
        let setup = || -> BoxedResult<Self> {
            let human = ctx.new_human(EntityPosition::Origin)?;
            let wall_count = 3;
            let bricks_needed = BRICKS_PER_WALL * wall_count;

            let mut brick_stack = None;
            let mut all_bricks = vec![];
            let mut randy = StdRng::seed_from_u64(847171);
            for _ in 0..bricks_needed {
                let brick = ctx.new_entity(
                    "core_brick_stone",
                    EntityPosition::Custom((randy.gen_range(2, 7), randy.gen_range(2, 7))),
                )?;
                all_bricks.push(brick);

                // ensure stackability is consistent
                ctx.simulation
                    .ecs
                    .component_mut::<StackableComponent>(brick)
                    .unwrap()
                    .max_count = MAX_STACKABILITY;

                let res = match brick_stack {
                    None => ctx
                        .simulation
                        .ecs
                        .helpers_containers()
                        .convert_to_stack(brick)
                        .map(|stack| {
                            brick_stack = Some(stack);
                        }),
                    Some(stack) => ctx
                        .simulation
                        .ecs
                        .helpers_containers()
                        .add_to_stack(stack, brick),
                };

                if let Err(err) = res {
                    if let ContainersError::StackError(ItemStackError::Full) = err {
                        brick_stack = None;
                    } else {
                        panic!("failed to stack bricks: {}", err);
                    }
                }
            }

            // create build jobs
            let societies = ctx.simulation.ecs.resource_mut::<Societies>();

            let soc = societies.new_society("People".to_owned()).unwrap();
            let society = societies.society_by_handle(soc).unwrap();
            ctx.simulation
                .ecs
                .add_now(human, SocietyComponent::new(soc))
                .unwrap();

            let walls = [(2, 2), (2, 8), (3, 4)]
                .iter()
                .copied()
                .map(|(x, y)| WorldPosition::from((x, y, 1)))
                .collect_vec();

            let build = Rc::new(BuildTemplate::new(
                vec![BuildMaterial::new(
                    CachedStr::from("core_brick_stone"),
                    NonZeroU16::new(BRICKS_PER_WALL).unwrap(),
                )],
                4,
                4,
                BlockType::StoneBrickWall,
            ));

            for pos in walls.iter() {
                society
                    .jobs_mut()
                    .submit(ctx.simulation.ecs, BuildThingJob::new(*pos, build.clone()));
            }

            Ok(Self {
                builder: human,
                remaining_builds: walls.len(),
                first_time_finishing: true,
                all_builds: walls,
                all_bricks,
            })
        };

        setup().into()
    }
}

declare_test!(GatherAndBuild);
