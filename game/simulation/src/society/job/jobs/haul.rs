use crate::activity::HaulTarget;
use crate::ecs::*;
use crate::item::{ContainedInComponent, HauledItemComponent};
use crate::job::SocietyTaskResult;
use crate::society::job::job::SocietyJobImpl;
use crate::society::job::SocietyTask;

use crate::{ContainerComponent, PhysicalComponent, TransformComponent};
use common::*;
use unit::world::{WorldPoint, WorldPosition};

/// Haul a thing to a position
// TODO differentiate hauling types, reasons and container choices e.g. to any container (choose in ai), to nearby a build project, to specific container
#[derive(Debug)]
pub struct HaulJob {
    entity: Entity,
    source: HaulTarget,
    target: HaulTarget,
}

impl HaulJob {
    pub fn with_target_position(
        thing: Entity,
        target: WorldPosition,
        world: &impl ComponentWorld,
    ) -> Option<Self> {
        let source = HaulTarget::with_entity(thing, world)?;

        let world = world.voxel_world();
        let world = world.borrow();

        // ensure target is accessible
        let _accessible = world.area(target).ok()?;

        let target = HaulTarget::Position(target);
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
        let source = HaulTarget::with_entity(thing, world)?;

        let target = HaulTarget::Container(container);
        Some(HaulJob {
            entity: thing,
            source,
            target,
        })
    }
}

impl SocietyJobImpl for HaulJob {
    fn populate_initial_tasks(&self, _: &EcsWorld, out: &mut Vec<SocietyTask>) {
        out.push(SocietyTask::Haul(self.entity, self.source, self.target));
    }

    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        mut completions: std::vec::Drain<(SocietyTask, SocietyTaskResult)>,
    ) -> Option<SocietyTaskResult> {
        debug_assert!(tasks.len() <= 1);

        // apply completion
        if let Some((task, result)) = completions.next() {
            debug!("haul task completed"; "task" => ?task, "result" => ?result);
            debug_assert_eq!(tasks.get(0).cloned(), Some(task), "unexpected completion");

            // ensure no more
            assert!(completions.next().is_none(), "single completion expected");

            // end job regardless of success or failure
            // TODO depends on error type?
            return Some(result);
        }

        // TODO fail early if no space left in container

        if world.has_component::<HauledItemComponent>(self.entity) {
            // skip checks if item is currently being hauled
            trace!("item is being hauled");
        } else {
            match self.target {
                HaulTarget::Position(target_pos) => {
                    let current_pos = match world.component::<TransformComponent>(self.entity) {
                        Ok(t) => &t.position,
                        Err(err) => {
                            debug!("hauled item is missing transform"; "item" => E(self.entity));
                            return Some(SocietyTaskResult::Failure(err.into()));
                        }
                    };

                    if WorldPoint::from(target_pos).is_almost(current_pos, 2.0) {
                        trace!("hauled item arrived at target");
                        return Some(SocietyTaskResult::Success);
                    }
                }
                HaulTarget::Container(target_container) => {
                    // check if arrived in the target container
                    match world.component::<ContainedInComponent>(self.entity) {
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
            SocietyTask::Haul(self.entity, self.source, self.target)
        )
    }
}
