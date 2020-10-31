use common::*;

use crate::ecs::{EcsWorld, Entity};
use crate::society::job::task::Task;
use crate::society::job::{BreakBlocksJob, HaulJob};
use crate::society::Society;
use crate::{ComponentWorld, WorldPositionRange};
use unit::world::WorldPosition;

pub trait Job: Display + Debug {
    /// Job is treated as finished if no tasks are produced
    fn outstanding_tasks(
        &mut self,
        world: &EcsWorld,
        society: &Society,
        out: &mut Vec<Task>,
    ) -> JobStatus;
}

#[derive(Debug)]
pub enum JobStatus {
    Finished,
    Ongoing,
    /// Finished if 0 tasks produced
    TaskDependent,
}

/// Declarative society command that will be resolved into a `Job`
#[derive(Debug)]
pub enum SocietyCommand {
    BreakBlocks(WorldPositionRange),
    HaulToPosition(Entity, WorldPosition),

    /// (thing, container)
    HaulIntoContainer(Entity, Entity),
}

impl SocietyCommand {
    pub fn into_job(self, world: &impl ComponentWorld) -> Result<Box<dyn Job>, Self> {
        use self::SocietyCommand::*;

        // TODO return a dyn error in result
        let job: Box<dyn Job> = match self {
            BreakBlocks(range) => Box::new(BreakBlocksJob::new(range)),
            HaulToPosition(e, pos) => {
                Box::new(HaulJob::with_target_position(e, pos, world).ok_or(self)?)
            }
            HaulIntoContainer(e, container) => {
                Box::new(HaulJob::with_target_container(e, container, world).ok_or(self)?)
            }
        };

        Ok(job)
    }
}
