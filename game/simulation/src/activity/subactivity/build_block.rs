use crate::ecs::ComponentGetError;

use crate::activity::context::ActivityContext;
use crate::job::{BuildDetails, BuildThingJob, SocietyJobHandle};

use crate::ComponentWorld;
use crate::{TransformComponent, WorldPosition};
use common::*;
use unit::world::WorldPoint;

#[derive(Debug, Error)]
pub enum BuildBlockError {
    #[error("Bad entity with no transform")]
    MissingTransform(#[from] ComponentGetError),

    #[error("Too far from block {target} to break it from {current}")]
    TooFar {
        current: WorldPoint,
        target: WorldPosition,
    },

    #[error("Failed to set block during completion")]
    CompletionFailed,

    #[error("Job not found or is not a build job")]
    InvalidJob(SocietyJobHandle),
}

#[derive(Default)]
pub struct BuildBlockSubactivity;

impl BuildBlockSubactivity {
    pub async fn build_block(
        &self,
        ctx: &ActivityContext,
        job: SocietyJobHandle,
        details: &BuildDetails,
    ) -> Result<(), BuildBlockError> {
        // check we are close enough
        let my_pos = ctx
            .world()
            .component::<TransformComponent>(ctx.entity())
            .map_err(BuildBlockError::MissingTransform)?
            .position;

        // TODO stop hardcoding distance check for block actions
        if my_pos.distance2(details.pos) > 5.0 {
            return Err(BuildBlockError::TooFar {
                current: my_pos,
                target: details.pos,
            });
        }

        // collect reserved materials
        let (materials, progress_details) = job
            .resolve_and_cast(ctx.world().resource(), |build_job: &BuildThingJob| {
                let materials = build_job.reserved_materials().map(Into::into).collect_vec();
                let progress = build_job.progress();

                (materials, progress)
            })
            .ok_or(BuildBlockError::InvalidJob(job))?;

        let helper = ctx
            .world()
            .helpers_building()
            .start_build(details.clone(), materials);
        // wait for that block to appear
        ctx.yield_now().await;

        loop {
            // TODO roll the dice for each step/hit/swing, e.g. injury

            // need to reresolve the job each time
            let new_progress = job
                .resolve_and_cast_mut(ctx.world().resource(), |build_job: &mut BuildThingJob| {
                    build_job.make_progress()
                })
                .ok_or(BuildBlockError::InvalidJob(job))?;

            if new_progress >= progress_details.total_steps_needed {
                break;
            }

            // TODO ensure we break out of this wait early if job is finished during
            ctx.wait(progress_details.progress_rate).await;
        }

        helper
            .complete_build(ctx.world())
            .map_err(|_| BuildBlockError::CompletionFailed)?;
        Ok(())
    }
}
