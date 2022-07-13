use std::sync::Arc;

use common::*;
use unit::space::length::{Length, Length2};
use unit::world::WorldPositionRange;
use world::loader::WorldTerrainUpdate;
use world_types::BlockType;

use crate::ecs::*;
use crate::event::DeathReason;
use crate::job::{BuildDetails, BuildThingJob, SocietyJobHandle};
use crate::render::UiElementComponent;
use crate::simulation::TerrainUpdatesRes;
use crate::{QueuedUpdates, SocietyComponent, TransformComponent,};

#[derive(common::derive_more::Deref, common::derive_more::DerefMut)]
pub struct EcsExtBuild<'w>(&'w EcsWorld);

impl EcsWorld {
    pub fn helpers_building(&self) -> EcsExtBuild {
        EcsExtBuild(self)
    }
}

pub struct BuildHelper {
    details: BuildDetails,
    reserved_materials: Arc<Vec<Entity>>,
}

impl<'w> EcsExtBuild<'w> {
    pub fn create_build(self, details: BuildDetails, job: SocietyJobHandle) {
        let pos = details.pos;
        self.0
            .resource::<QueuedUpdates>()
            .queue("create wip build", move |world| {
                // spawn entity for wip job
                let wip_size = Length::blocks(1.0);
                let wip = Entity::from(
                    world
                        .create_entity()
                        .with(TransformComponent::new(pos.centred()))
                        .with(UiElementComponent {
                            build_job: job,
                            size: Length2::new(wip_size, wip_size),
                        })
                        .with(KindComponent::from_display(&format_args!(
                            "{} (in progress)",
                            details.target
                        )))
                        .with(SocietyComponent::new(job.society()))
                        .build(),
                );

                // kill on failure
                let bomb = EntityBomb::new(wip, &world, DeathReason::Unknown);

                // register ui entity with job
                job.resolve_and_cast_mut(world.resource(), |build_job: &mut BuildThingJob| {
                    trace!("spawned ui element entity for build job"; "ui" => wip, "job" => ?job);
                    build_job.set_ui_element(wip);
                })
                .ok_or("invalid build job")?;

                bomb.defuse();
                Ok(())
            });
    }

    pub fn start_build(
        self,
        details: BuildDetails,
        reserved_materials: Vec<Entity>,
    ) -> BuildHelper {
        let reserved_materials = Arc::new(reserved_materials);
        let consumed_materials = reserved_materials.clone();
        self.0
            .resource::<QueuedUpdates>()
            .queue("start wip build", move |world| {
                // consume materials
                world
                    .helpers_comps()
                    .consume_materials_for_job(&consumed_materials);
                // TODO consume materials incrementally as progress is made

                Ok(())
            });

        BuildHelper {
            details,
            reserved_materials,
        }
    }
}

impl BuildHelper {
    pub fn complete_build(self, ecs: &EcsWorld) -> Result<(), ()> {
        let world = ecs.voxel_world();
        let prev_block = {
            let world = world.borrow();
            world.block(self.details.pos).map(|b| b.block_type())
        };

        if !matches!(prev_block, Some(BlockType::Air)) {
            warn!("unexpected block type when finishing build"; "block" => %self.details.pos, "current" => ?prev_block);
            return Err(());
        }

        // place the block in the world
        ecs.resource_mut::<TerrainUpdatesRes>()
            .push(WorldTerrainUpdate::new(
                WorldPositionRange::with_single(self.details.pos),
                self.details.target,
            ));

        // destroy consumed materials
        let materials = self.reserved_materials;
        debug!("build job was completed, queueing material destruction"; "materials" => ?*materials);
        ecs.resource::<QueuedUpdates>().queue(
            "destroying materials for completed build",
            move |world| {
                world.kill_entities(&materials, DeathReason::CompletedBuild);
                Ok(())
            },
        );

        // wip entity will be killed when the job is destroyed

        Ok(())
    }
}
