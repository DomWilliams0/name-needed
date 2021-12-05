use crate::ecs::ComponentGetError;
use specs::WorldExt;

use crate::activity::context::ActivityContext;
use crate::job::{BuildDetails, BuildThingJob, SocietyJobHandle};
use crate::queued_update::QueuedUpdates;
use crate::{ComponentWorld, Entity, TerrainUpdatesRes, WorldPositionRange, WorldTerrainUpdate};
use crate::{TransformComponent, WorldPosition};
use common::*;
use unit::world::WorldPoint;
use world::block::BlockType;

#[derive(Debug, Error)]
pub enum BuildBlockError {
    #[error("Bad entity with no transform")]
    MissingTransform(#[from] ComponentGetError),

    #[error("Too far from block {target} to break it from {current}")]
    TooFar {
        current: WorldPoint,
        target: WorldPosition,
    },

    #[error("Block is invalid or non-air")]
    BadBlock,

    #[error("Job not found in society")]
    JobNotFound(SocietyJobHandle),

    #[error("Job is not a build job")]
    InvalidJob(SocietyJobHandle),
}

#[derive(Default)]
pub struct BuildBlockSubactivity;

struct MaterialsBomb<'a> {
    materials: Vec<Entity>,
    completed: bool,
    ctx: &'a ActivityContext,
    job_pos: WorldPosition,
}

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

        // "consume" materials on start
        let mut materials = MaterialsBomb {
            materials: resolve_job(ctx, job)?,
            completed: false,
            ctx,
            job_pos: details.pos,
        };

        queue_material_consumption(ctx.world().resource(), &materials.materials);
        // TODO consume materials incrementally as progress is made

        // TODO add some in-progress block to the world, instead of just being air while working

        // TODO progression rate depends on job
        let progress_steps = 8;
        let progress_rate = 4; // ticks between step

        for _ in 0..progress_steps {
            // TODO roll the dice for each step/hit/swing, e.g. injury

            ctx.wait(progress_rate).await;
        }

        // work is complete
        materials.completed = true;

        // place the block in the world
        let world = ctx.world().voxel_world();
        let world = world.borrow();
        match world.block(details.pos).map(|b| b.block_type()) {
            Some(BlockType::Air) => { /* nice */ }
            Some(current) if current == details.target => {
                trace!("block is already build target type"; "block" => %details.pos, "type" => ?current);
            }
            Some(_) | None => {
                trace!("cannot build non-air or invalid block"; "block" => %details.pos);
                return Err(BuildBlockError::BadBlock);
            }
        };

        let terrain_updates = ctx.world().resource_mut::<TerrainUpdatesRes>();
        terrain_updates.push(WorldTerrainUpdate::new(
            WorldPositionRange::with_single(details.pos),
            details.target,
        ));

        Ok(())
    }
}

fn resolve_job(
    ctx: &ActivityContext,
    job: SocietyJobHandle,
) -> Result<Vec<Entity>, BuildBlockError> {
    // find job in society
    let job_ref = job
        .resolve(ctx.world().resource())
        .ok_or(BuildBlockError::JobNotFound(job))?;

    // cast job to a build job
    let job_ref = job_ref.borrow();
    let build_job = job_ref
        .cast::<BuildThingJob>()
        .ok_or(BuildBlockError::InvalidJob(job))?;

    Ok(build_job.reserved_materials().map(Into::into).collect())
}

fn queue_material_consumption(updates: &QueuedUpdates, materials: &[Entity]) {
    let materials_cloned = Vec::from(materials);
    updates.queue("consume build materials", move |world| {
        world
            .helpers_comps()
            .consume_materials_for_job(&materials_cloned);
        Ok(())
    });
}

impl Drop for MaterialsBomb<'_> {
    fn drop(&mut self) {
        let updates = self.ctx.world().resource::<QueuedUpdates>();
        let materials = std::mem::take(&mut self.materials);

        if self.completed {
            debug!("build job was completed, queueing material destruction"; "materials" => ?materials);
            updates.queue("destroying materials for completed build", |world| {
                for material in materials {
                    world.kill_entity(material);
                }

                Ok(())
            });
        } else {
            debug!("build job was interrupted, dropping unconsumed materials"; "materials" => ?materials);
            // TODO some materials should be consumed depending on the progress before interrupting
            // TODO do this on destruction of the in-progress block instead of interrupting

            let job_pos = self.job_pos;
            updates.queue("dropping materials for interrupted build", move |world| {
                world
                    .helpers_comps()
                    .unconsume_materials_for_job(&materials, job_pos.centred());
                Ok(())
            });
        }
    }
}
