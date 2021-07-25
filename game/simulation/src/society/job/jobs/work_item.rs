use crate::job::job::{CompletedTasks, SocietyJobImpl};
use crate::job::{SocietyTask, SocietyTaskResult};
use crate::society::work_item::WorkItemRef;
use crate::EcsWorld;
use common::*;

/// Go to a work item and actively work on it, assuming its dependencies are satisfied
#[derive(Debug)]
pub struct WorkOnWorkItemJob(pub WorkItemRef);

impl SocietyJobImpl for WorkOnWorkItemJob {
    fn populate_initial_tasks(&self, world: &EcsWorld, out: &mut Vec<SocietyTask>) {
        out.push(SocietyTask::WorkOnWorkItem(self.0.clone()));
    }

    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: CompletedTasks,
    ) -> Option<SocietyTaskResult> {
        // TODO check work item dependencies
        // TODO manage completions
        // TODO remove when complete
        None
    }
}

impl Display for WorkOnWorkItemJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
