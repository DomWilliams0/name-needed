use crate::activity::HaulTarget;
use crate::ecs::*;
use crate::item::{ContainedInComponent, HauledItemComponent};
use crate::job::SocietyTaskResult;
use crate::society::job::job2::SocietyJobImpl;
use crate::society::job::SocietyTask;
use crate::society::Society;
use crate::TransformComponent;
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
    fn populate_initial_tasks(&self, out: &mut Vec<SocietyTask>) {
        out.push(SocietyTask::Haul(self.entity, self.source, self.target));
    }

    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: std::vec::Drain<(SocietyTask, SocietyTaskResult)>,
    ) -> Option<SocietyTaskResult> {
        // TODO apply completions
        assert_eq!(completions.count(), 0, "todo");

        if world.has_component::<HauledItemComponent>(self.entity) {
            // skip checks if item is currently being hauled
            trace!("item is being hauled");
        } else {
            match self.target {
                HaulTarget::Position(target_pos) => {
                    let current_pos = match world.component::<TransformComponent>(self.entity) {
                        Ok(t) => &t.position,
                        Err(_) => {
                            debug!("hauled item is missing transform"; "item" => E(self.entity));
                            return Some(SocietyTaskResult::Failure);
                        }
                    };

                    if WorldPoint::from(target_pos).is_almost(current_pos, 2.0) {
                        trace!("hauled item arrived at target");
                        return Some(SocietyTaskResult::Success);
                    }
                }
                HaulTarget::Container(target_container) => {
                    match world.component::<ContainedInComponent>(self.entity) {
                        Ok(ContainedInComponent::Container(c)) if *c == target_container => {
                            trace!("hauled item arrived in target container");
                            return Some(SocietyTaskResult::Success);
                        }
                        _ => {}
                    };
                }
            };
        }

        // keep single haul task
        None
    }
}

impl Display for HaulJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Haul {} to {}", E(self.entity), self.target)
    }
}
