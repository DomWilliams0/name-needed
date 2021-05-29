use crate::ecs::EcsWorld;
use crate::society::job::job::JobStatus;
use crate::society::job::{Job, SocietyTask};
use crate::society::Society;
use crate::{BlockType, ComponentWorld, WorldPositionRange};
use common::derive_more::*;
use common::*;

#[derive(Constructor, Debug)]
pub struct BreakBlocksJob(WorldPositionRange);

impl Job for BreakBlocksJob {
    fn outstanding_tasks(
        &mut self,
        world: &EcsWorld,
        _: &Society,
        out: &mut Vec<SocietyTask>,
    ) -> JobStatus {
        let voxel_world = world.voxel_world();
        let voxel_world = voxel_world.borrow();

        // only queue blocks that are not air and are reachable
        out.extend(
            voxel_world
                .filter_reachable_blocks_in_range(&self.0, |bt| bt != BlockType::Air)
                .map(SocietyTask::BreakBlock),
        );

        JobStatus::TaskDependent
    }
}

impl Display for BreakBlocksJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Break {} blocks in range {:?}", self.0.count(), self.0)
    }
}
