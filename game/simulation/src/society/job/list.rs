use std::ops::Deref;

use common::*;
use world::WorldRef;

use crate::ecs::Entity;
use crate::simulation::Tick;
use crate::society::job::job::Job;
use crate::society::job::task::Task;
use crate::society::job::TaskReservations;

#[derive(Default)]
pub struct JobList {
    jobs: ActiveJobs,

    /// The last tick that `active_tasks` represents, used for caching
    last_update: Tick,

    reserved: TaskReservations,
}

#[derive(Default)]
struct ActiveJobs {
    active_jobs: Vec<Box<dyn Job>>,

    active_tasks: Vec<Task>,
}

impl ActiveJobs {
    fn collect_tasks(&mut self, world: &WorldRef) -> &[Task] {
        self.active_tasks.clear();

        // TODO reuse allocation
        let mut finished = Vec::new();

        for (i, job) in self.active_jobs.iter_mut().enumerate() {
            let len_before = self.active_tasks.len();
            job.outstanding_tasks(world, &mut self.active_tasks);
            let task_count = self.active_tasks.len() - len_before;

            if task_count == 0 {
                debug!("job produced no tasks, marking as finished"; "job" => ?job);
                finished.push(i);
            } else {
                debug!("job produced {count} tasks", count = task_count; "job" => ?job);
            }
        }

        for finished in finished {
            self.active_jobs.swap_remove(finished);
        }

        &self.active_tasks
    }
}

impl JobList {
    pub fn submit(&mut self, job: Box<dyn Job>) {
        self.jobs.active_jobs.push(job);
    }

    /// Filters out reserved tasks by entities other than the given.
    /// Returns (if a cache was returned, tasks)
    pub fn collect_cached_tasks_for(
        &mut self,
        this_tick: Tick,
        world: &WorldRef,
        entity: Entity,
    ) -> (bool, impl Iterator<Item = Task> + '_) {
        let (cached, tasks) = if self.last_update == this_tick {
            // already updated this tick, return the same results
            (true, self.jobs.active_tasks.as_slice())
        } else {
            // new tick, new me
            self.last_update = this_tick;
            (false, self.jobs.collect_tasks(world))
        };

        // filter out reserved tasks
        let reserved = &self.reserved;
        let tasks = tasks
            .iter()
            .cloned()
            .filter(move |t| reserved.is_available_to(t, entity));

        (cached, tasks)
    }

    pub fn iter_jobs(&self) -> impl Iterator<Item = &dyn Job> {
        self.jobs.active_jobs.iter().map(|j| j.deref())
    }

    pub fn count(&self) -> usize {
        self.jobs.active_jobs.len()
    }

    pub fn reserve_task(&mut self, entity: Entity, task: Task) {
        let (prev_task, prev_reserver) = self.reserved.reserve(entity, task.clone());
        debug!("reserved task, unreserving previous";
            "task" => ?task, "prev_task" => ?prev_task,
            "prev_reserver" => ?prev_reserver
        );
    }

    pub fn unreserve_task(&mut self, entity: Entity) {
        if let Some(task) = self.reserved.unreserve(entity) {
            debug!("unreserved society task"; "task" => ?task)
        }
    }
}
