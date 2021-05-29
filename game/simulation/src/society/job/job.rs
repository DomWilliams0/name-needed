use common::*;

use crate::ecs::{EcsWorld, Entity};
use crate::job::job2::SocietyJob;
use crate::job::SocietyJobRef;
use crate::society::job::task::SocietyTask;
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
    pub fn into_job(self, world: &impl ComponentWorld) -> Result<SocietyJobRef, Self> {
        use self::SocietyCommand::*;

        // TODO return a dyn error in result
        Ok(match self {
            BreakBlocks(range) => todo!(), // TODO break blocks
            HaulToPosition(e, pos) => {
                SocietyJob::create(HaulJob::with_target_position(e, pos, world).ok_or(self)?)
            }
            HaulIntoContainer(e, container) => {
                SocietyJob::create(HaulJob::with_target_container(e, container, world).ok_or(self)?)
            }
        })
    }
}
