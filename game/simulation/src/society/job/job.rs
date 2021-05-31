use common::*;

use crate::ecs::{EcsWorld, Entity};
use crate::job::job2::SocietyJob;
use crate::job::SocietyJobRef;
use crate::society::job::task::SocietyTask;
use crate::society::job::{BreakBlocksJob, HaulJob};
use crate::society::Society;
use crate::WorldPositionRange;
use unit::world::WorldPosition;

pub trait Job: Display + Debug {
    /// Job is treated as finished if no tasks are produced
    fn outstanding_tasks(
        &mut self,
        world: &EcsWorld,
        society: &Society,
        out: &mut Vec<SocietyTask>,
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
    pub fn into_job(self, world: &EcsWorld) -> Result<SocietyJobRef, Self> {
        use self::SocietyCommand::*;

        macro_rules! job {
            ($job:expr) => {
                Ok(SocietyJob::create(world, $job))
            };
        }

        // TODO return a dyn error in result
        match self {
            BreakBlocks(range) => job!(BreakBlocksJob::new(range)),
            HaulToPosition(e, pos) => {
                job!(HaulJob::with_target_position(e, pos, world).ok_or(self)?)
            }
            HaulIntoContainer(e, container) => {
                job!(HaulJob::with_target_container(e, container, world).ok_or(self)?)
            }
        }
    }
}
