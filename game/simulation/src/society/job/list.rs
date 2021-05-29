use std::ops::Deref;

use common::*;

use crate::ecs::{EcsWorld, Entity};
use crate::job::job::JobStatus;
use crate::simulation::Tick;
use crate::society::job::job::Job;
use crate::society::job::task::SocietyTask;
use crate::society::job::TaskReservations;
use crate::society::Society;

#[derive(Default)]
pub struct JobList {
    jobs: ActiveJobs,

    /// The last tick that `active_tasks` represents, used for caching
    last_update: Tick,

    reserved: TaskReservations,
}

#[derive(Default)]
struct ActiveJobs {
    // TODO use dynstack instead of boxes for society jobs
    active_jobs: Vec<Box<dyn Job>>,

    active_tasks: Vec<SocietyTask>,
}

impl ActiveJobs {
    fn collect_tasks(&mut self, world: &EcsWorld, society: &Society) -> &[SocietyTask] {
        self.active_tasks.clear();

        // TODO reuse allocation
        let mut finished_indices = Vec::new();

        for (i, job) in self.active_jobs.iter_mut().enumerate() {
            let len_before = self.active_tasks.len();
            let status = job.outstanding_tasks(world, society, &mut self.active_tasks);
            let task_count = self.active_tasks.len() - len_before;

            let finished = match status {
                JobStatus::TaskDependent => task_count == 0,
                JobStatus::Finished => true,
                JobStatus::Ongoing => false,
            };

            if finished {
                debug!("job is finished"; "job" => ?job, "status" => ?status);
                finished_indices.push(i);
            } else {
                debug!("job produced {count} tasks", count = task_count; "job" => ?job);
            }
        }

        remove_indices(&mut self.active_jobs, &finished_indices);

        &self.active_tasks
    }
}

/// Indices must be in order. Does not preserve original order
fn remove_indices<T>(vec: &mut Vec<T>, to_remove: &[usize]) {
    // ensure sorted and not too long
    debug_assert!(
        to_remove.iter().tuple_windows().all(|(a, b)| a < b),
        "indices are not sorted"
    );
    debug_assert!(to_remove.len() <= vec.len());

    let mut end = (vec.len() as isize) - 1;
    for idx in to_remove.iter().rev() {
        vec.swap(*idx, end as usize);
        end -= 1;
    }

    vec.truncate((end + 1) as usize);
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
        world: &EcsWorld,
        society: &Society,
        entity: Entity,
    ) -> (bool, impl Iterator<Item = SocietyTask> + '_) {
        let (cached, tasks) = if self.last_update == this_tick {
            // already updated this tick, return the same results
            (true, self.jobs.active_tasks.as_slice())
        } else {
            // new tick, new me
            self.last_update = this_tick;
            (false, self.jobs.collect_tasks(world, society))
        };

        // filter out reserved tasks
        // TODO dont recalculate all unreserved tasks every tick for every entity
        //  - we have the context in the system, maintain a list per tick and add/rm tasks to it as
        //  they are reserved by entities one by one
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

    pub fn reserve_task(&mut self, entity: Entity, task: SocietyTask) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_indices_from_vec() {
        fn check(mut input: Vec<i32>, remove: Vec<usize>, mut output: Vec<i32>) {
            remove_indices(&mut input, &remove);
            input.sort_unstable();
            output.sort_unstable();
            assert_eq!(input, output);
        }

        check(vec![10, 11, 12, 13], vec![0, 2, 3], vec![11]);
        check(vec![10, 11, 12, 13], vec![0], vec![11, 12, 13]);
        check(vec![10, 11, 12, 13], vec![1], vec![10, 12, 13]);
        check(vec![10, 11, 12, 13], vec![2], vec![10, 11, 13]);
        check(vec![10, 11, 12, 13], vec![3], vec![10, 11, 12]);
        check(vec![10, 11, 12, 13], vec![0, 1, 2, 3], vec![]);
    }
}
