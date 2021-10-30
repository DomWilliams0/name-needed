use crate::ecs::{CachedWorldRef, EcsWorld};
use crate::job::job::{CompletedTasks, SocietyJobImpl};
use crate::job::SocietyTaskResult;
use crate::society::job::SocietyTask;
use crate::{
    BlockType, ComponentWorld, InnerWorldRef, WorldPosition, WorldPositionRange, WorldRef,
};
use common::derive_more::*;
use common::*;
use std::hint::unreachable_unchecked;

#[derive(Constructor, Debug)]
pub struct BuildBlockJob {
    // TODO support multiple blocks
    block: WorldPosition,
    target_type: BlockType,
}

impl SocietyJobImpl for BuildBlockJob {
    fn populate_initial_tasks(&self, world: &EcsWorld, out: &mut Vec<SocietyTask>) {
        let voxel_world = world.voxel_world();
        let voxel_world = voxel_world.borrow();

        // only queue if not already the target block
        let blocks = WorldPositionRange::with_single(self.block);
        out.extend(
            voxel_world
                .filter_reachable_blocks_in_range(&blocks, |bt| bt != self.target_type)
                .map(|pos| SocietyTask::Build(pos, self.target_type)),
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
                // check if block is now the target block
                let world = voxel_world.get();
                let (block_pos, tgt_bt) = match task {
                    SocietyTask::Build(p, bt) => (*p, *bt),
                    _ => unreachable!(),
                };

                !world
                    .block(block_pos)
                    .map(|b| b.block_type() == tgt_bt)
                    .unwrap_or(true)
            }
        });

        // determine job result from number of tasks left
        None
    }
}

impl Display for BuildBlockJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Build {} at {}", self.target_type, self.block)
    }
}
