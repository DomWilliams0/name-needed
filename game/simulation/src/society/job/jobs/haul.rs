use common::*;
use unit::world::WorldPoint;

use crate::activity::{HaulSource, HaulTarget};
use crate::ecs::*;
use crate::item::{ContainedInComponent, HauledItemComponent};
use crate::job::job::CompletedTasks;
use crate::job::task::HaulSocietyTask;
use crate::job::{SocietyJobHandle, SocietyTaskResult};
use crate::society::job::job::SocietyJobImpl;
use crate::society::job::SocietyTask;
use crate::{ContainerComponent, PhysicalComponent, TransformComponent};

/// Haul a thing to a position
// TODO differentiate hauling types, reasons and container choices e.g. to any container (choose in ai), to nearby a build project, to specific container
#[derive(Debug)]
pub struct HaulJob {
    entity: Entity,
    source: HaulSource,
    target: HaulTarget,
}

impl HaulJob {
    pub fn with_target_position(
        thing: Entity,
        target: WorldPoint,
        world: &impl ComponentWorld,
    ) -> Option<Self> {
        let source = HaulSource::with_entity(thing, world)?;

        let world = world.voxel_world();
        let world = world.borrow();

        // ensure target is accessible
        let _accessible = world.area(target.floor()).ok()?;

        let target = HaulTarget::Drop(target);
        Some(HaulJob {
            entity: thing,
            source,
            target,
        })
    }

    pub fn with_target_container(
        thing: Entity,
        container: Entity,
        world: &impl ComponentWorld,
    ) -> Option<Self> {
        let source = HaulSource::with_entity(thing, world)?;

        let target = HaulTarget::Container(container);
        Some(HaulJob {
            entity: thing,
            source,
            target,
        })
    }
}

impl SocietyJobImpl for HaulJob {
    fn populate_initial_tasks(
        &mut self,
        _: &EcsWorld,
        out: &mut Vec<SocietyTask>,
        _: SocietyJobHandle,
    ) {
        out.push(SocietyTask::haul(self.entity, self.source, self.target));
    }

    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: CompletedTasks,
    ) -> Option<SocietyTaskResult> {
        debug_assert!(tasks.len() <= 1);

        // ignore failures
        let completion = completions
            .iter()
            .find(|(_, res)| matches!(res, SocietyTaskResult::Success));

        // apply completion
        if let Some((task, result)) = completion {
            debug!("haul task completed"; "task" => ?task, "result" => ?result);
            debug_assert_eq!(tasks.get(0), Some(task), "unexpected successful completion");
            return Some(SocietyTaskResult::Success);
        }

        // TODO fail early if no space left in container

        if world.has_component::<HauledItemComponent>(self.entity) {
            // skip checks if item is currently being hauled
            trace!("item is being hauled");
        } else {
            match self.target {
                HaulTarget::Drop(target_pos) => {
                    let current_pos = match world.component::<TransformComponent>(self.entity) {
                        Ok(t) => t.position,
                        Err(err) => {
                            debug!("hauled item is missing transform"; "item" => self.entity);
                            return Some(SocietyTaskResult::Failure(err.into()));
                        }
                    };

                    if target_pos.is_almost(&current_pos, 2.0) {
                        trace!("hauled item arrived at target");
                        return Some(SocietyTaskResult::Success);
                    }
                }
                HaulTarget::Container(target_container) => {
                    // check if arrived in the target container
                    match world
                        .component::<ContainedInComponent>(self.entity)
                        .as_deref()
                    {
                        Ok(ContainedInComponent::Container(c)) if *c == target_container => {
                            trace!("hauled item arrived in target container");
                            return Some(SocietyTaskResult::Success);
                        }
                        _ => {}
                    };

                    // check there is space within the target
                    if let Err(err) =
                        ensure_item_fits_in_container(target_container, self.entity, world)
                    {
                        trace!("hauled item cannot fit in target container");
                        return Some(SocietyTaskResult::Failure(err));
                    }
                }
            };
        }

        // keep single haul task
        None
    }

    crate::as_any_impl!();
}

fn ensure_item_fits_in_container(
    container: Entity,
    item: Entity,
    world: &EcsWorld,
) -> BoxedResult<()> {
    let container = world.component::<ContainerComponent>(container)?;
    let item_physical = world.component::<PhysicalComponent>(item)?;

    container
        .container
        .fits(item_physical.size, item_physical.volume)?;

    Ok(())
}

impl Display for HaulJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // delegate to avoid duplication
        write!(
            f,
            "{}",
            HaulSocietyTask {
                item: self.entity,
                src: self.source,
                dst: self.target,
            },
        )
    }
}
