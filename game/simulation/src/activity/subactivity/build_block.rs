use crate::ecs::ComponentGetError;

use std::sync::Arc;

use crate::activity::context::ActivityContext;
use crate::job::{BuildDetails, BuildThingJob, SocietyJobHandle};

use crate::{ComponentWorld, Entity};
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

    #[error("Job not found in society")]
    JobNotFound(SocietyJobHandle),

    #[error("Job is not a build job")]
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
        let materials = resolve_job_materials(ctx, job)?;

        let helper = ctx
            .world()
            .helpers_building()
            .start_build(details.clone(), job, materials);
        // wait for that block to appear
        ctx.yield_now().await;

        // TODO progression rate depends on job
        let progress_steps = 8;
        let progress_rate = 4; // ticks between step

        for _ in 0..progress_steps {
            // TODO roll the dice for each step/hit/swing, e.g. injury

            ctx.wait(progress_rate).await;
        }

        helper
            .end_build(ctx.world())
            .map_err(|_| BuildBlockError::CompletionFailed)?;
        Ok(())
    }
}

fn resolve_job_materials(
    ctx: &ActivityContext,
    job: SocietyJobHandle,
) -> Result<Arc<Vec<Entity>>, BuildBlockError> {
    // find job in society
    let job_ref = job
        .resolve(ctx.world().resource())
        .ok_or(BuildBlockError::JobNotFound(job))?;

    // cast job to a build job
    let job_ref = job_ref.borrow();
    let build_job = job_ref
        .cast::<BuildThingJob>()
        .ok_or(BuildBlockError::InvalidJob(job))?;

    let materials = build_job.reserved_materials().map(Into::into).collect_vec();
    Ok(Arc::new(materials))
}
