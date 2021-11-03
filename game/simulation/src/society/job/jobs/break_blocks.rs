use crate::ecs::{CachedWorldRef, EcsWorld};
use crate::job::job::{CompletedTasks, SocietyJobImpl};
use crate::job::{SocietyJobHandle, SocietyTaskResult};
use crate::society::job::SocietyTask;
use crate::{BlockType, ComponentWorld, InnerWorldRef, WorldPositionRange, WorldRef};
use common::derive_more::*;
use common::*;

#[derive(Constructor, Debug)]
pub struct BreakBlocksJob(WorldPositionRange);

impl SocietyJobImpl for BreakBlocksJob {
    fn populate_initial_tasks(
        &self,
        world: &EcsWorld,
        out: &mut Vec<SocietyTask>,
        _: SocietyJobHandle,
    ) {
        let voxel_world = world.voxel_world();
        let voxel_world = voxel_world.borrow();

        // only queue blocks that are not air and are reachable
        out.extend(
            voxel_world
                .filter_reachable_blocks_in_range(&self.0, |bt| bt != BlockType::Air)
                .map(SocietyTask::BreakBlock),
        );
    }

    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: CompletedTasks,
    ) -> Option<SocietyTaskResult> {
        // obtain world ref lazily only once
        let mut voxel_world = CachedWorldRef::new(world);

        tasks.retain(|task| {
            if completions.iter().any(|(t, _)| t == task) {
                // task completed, remove it
                false
            } else {
                // check if block is now air for any other reason
                let world = voxel_world.get();
                let block_pos = match task {
                    SocietyTask::BreakBlock(p) => *p,
                    _ => unreachable!(),
                };

                !world
                    .block(block_pos)
                    .map(|b| b.block_type() == BlockType::Air)
                    .unwrap_or(true)
            }
        });

        // determine job result from number of tasks left
        None
    }
}

impl Display for BreakBlocksJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // TODO add display impl for WorldPositionRange
        write!(f, "Break {} blocks in range {:?}", self.0.count(), self.0)
    }
}
