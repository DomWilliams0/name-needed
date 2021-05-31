use crate::ecs::EcsWorld;
use crate::job::job2::SocietyJobImpl;
use crate::job::SocietyTaskResult;
use crate::society::job::SocietyTask;
use crate::{BlockType, ComponentWorld, InnerWorldRef, WorldPositionRange, WorldRef};
use common::derive_more::*;
use common::*;
use std::hint::unreachable_unchecked;

#[derive(Constructor, Debug)]
pub struct BreakBlocksJob(WorldPositionRange);

impl SocietyJobImpl for BreakBlocksJob {
    fn populate_initial_tasks(&self, world: &EcsWorld, out: &mut Vec<SocietyTask>) {
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
        completions: std::vec::Drain<(SocietyTask, SocietyTaskResult)>,
    ) -> Option<SocietyTaskResult> {
        // obtain world ref lazily only once
        let mut voxel_world = LazyWorldRef::new(world);

        tasks.retain(|task| {
            if completions.as_slice().iter().any(|(t, _)| t == task) {
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

struct LazyWorldRef<'a> {
    // TODO move to world, and cache current slab/chunk for contiguous queries
    ecs: &'a EcsWorld,
    voxel_world: Option<(WorldRef, InnerWorldRef<'a>)>,
}

impl<'a> LazyWorldRef<'a> {
    pub fn new(ecs: &'a EcsWorld) -> Self {
        Self {
            ecs,
            voxel_world: None,
        }
    }

    pub fn get(&mut self) -> &'_ InnerWorldRef<'a> {
        if self.voxel_world.is_none() {
            // init world ref and store
            let world = self.ecs.voxel_world();
            let world_ref = world.borrow();

            // safety: ref lives as long as self
            let world_ref =
                unsafe { std::mem::transmute::<InnerWorldRef, InnerWorldRef>(world_ref) };
            self.voxel_world = Some((world, world_ref));
        }

        match self.voxel_world.as_ref() {
            Some((_, w)) => w,
            _ => {
                // safety: unconditionally initialised
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Soundness confirmed by miri
    #[test]
    fn lazy_ecs() {
        let mut ecs = EcsWorld::new();
        ecs.insert(WorldRef::default());

        let mut lazy = LazyWorldRef::new(&ecs);

        let a = lazy.get();
        let b = lazy.get();
    }
}
