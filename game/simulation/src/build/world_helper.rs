use crate::ecs::*;
use crate::job::{BuildDetails, SocietyJobHandle};
use crate::{AssociatedBlockData, QueuedUpdates};
use common::*;
use std::sync::Arc;
use unit::world::WorldPositionRange;
use world::block::BlockType;
use world::loader::{TerrainUpdatesRes, WorldTerrainUpdate};

#[derive(common::derive_more::Deref, common::derive_more::DerefMut)]
pub struct EcsExtBuild<'w>(&'w EcsWorld);

impl EcsWorld {
    pub fn helpers_building(&self) -> EcsExtBuild {
        EcsExtBuild(self)
    }
}

pub struct BuildHelper {
    details: BuildDetails,
    job: SocietyJobHandle,
}

impl<'w> EcsExtBuild<'w> {
    pub fn start_build(
        self,
        details: BuildDetails,
        job: SocietyJobHandle,
        reserved_materials: Arc<Vec<Entity>>,
    ) -> BuildHelper {
        // add special wip block to track progress, and consume materials
        // TODO should this be an entity instead? it can be rendered differently
        self.0
            .resource_mut::<TerrainUpdatesRes>()
            .push(WorldTerrainUpdate::new(
                WorldPositionRange::with_single(details.pos),
                BlockType::IncompleteBuild,
            ));

        let pos = details.pos;
        self.0
            .resource::<QueuedUpdates>()
            .queue("start wip build", move |world| {
                // consume materials
                world
                    .helpers_comps()
                    .consume_materials_for_job(&reserved_materials);
                // TODO consume materials incrementally as progress is made

                // block update
                let w = world.voxel_world();
                let mut w = w.borrow_mut();
                w.set_associated_block_data(
                    pos,
                    AssociatedBlockData::BuildJobWip {
                        build: job,
                        reserved_materials,
                    },
                );

                Ok(())
            });

        BuildHelper { details, job }
    }
}

impl BuildHelper {
    pub fn end_build(self, ecs: &EcsWorld) -> Result<(), ()> {
        let world = ecs.voxel_world();
        let prev_block = {
            let world = world.borrow();
            world.block(self.details.pos).map(|b| b.block_type())
        };

        if let Some(BlockType::IncompleteBuild) = prev_block { /* nice */
        } else {
            warn!("unexpected block type when finishing build"; "block" => %self.details.pos, "current" => ?prev_block);
            return Err(());
        }

        // place the block in the world
        ecs.resource_mut::<TerrainUpdatesRes>()
            .push(WorldTerrainUpdate::new(
                WorldPositionRange::with_single(self.details.pos),
                self.details.target,
            ));

        // remove wip associated data
        let ass_data = {
            let mut world = world.borrow_mut();
            world.remove_associated_block_data(self.details.pos)
        };

        let materials = match ass_data {
            Some(AssociatedBlockData::BuildJobWip {
                reserved_materials,
                build,
            }) if build == self.job => reserved_materials,
            other => {
                warn!("removed unexpected associated data from newly built block"; "type" => ?self.details.target, "data" => ?other);
                return Err(());
            }
        };

        // destroy consumed materials
        debug!("build job was completed, queueing material destruction"; "materials" => ?*materials);
        ecs.resource::<QueuedUpdates>().queue(
            "destroying materials for completed build",
            move |world| {
                for material in &*materials {
                    world.kill_entity(*material);
                }

                Ok(())
            },
        );

        Ok(())
    }
}
