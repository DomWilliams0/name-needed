use crate::society::job::task::Task;
use common::derive_more::*;
use common::*;
use unit::world::WorldPositionRange;
use world::block::BlockType;
use world::WorldRef;

pub trait Job: Display + Debug {
    fn outstanding_tasks(&mut self, world: &WorldRef, out: &mut Vec<Task>);
}

#[derive(Constructor, Debug)]
pub struct BreakBlocksJob(WorldPositionRange);

impl Job for BreakBlocksJob {
    fn outstanding_tasks(&mut self, world: &WorldRef, out: &mut Vec<Task>) {
        let world = world.borrow();

        // only queue blocks that are not air and are reachable
        out.extend(
            world
                .filter_reachable_blocks_in_range(&self.0, |bt| bt != BlockType::Air)
                .map(Task::BreakBlock),
        );
    }
}

impl Display for BreakBlocksJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Break {} blocks in range {:?}", self.0.count(), self.0)
    }
}
