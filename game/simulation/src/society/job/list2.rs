use crate::ecs::E;
use crate::job::job2::{SocietyJob, SocietyJobImpl};
use crate::job::SocietyTask;
use crate::simulation::Tick;
use crate::society::job::job2::SocietyJobRef;
use crate::{EcsWorld, Entity};
use common::*;
use std::collections::HashSet;

#[derive(Debug)] // TODO implement manually
pub struct SocietyJobList {
    jobs: Vec<SocietyJobRef>,
    reservations: SocietyTaskReservations,
    last_update: Tick,

    /// Pretty hacky way to prevent Job indices changing
    no_more_jobs_temporarily: bool,
}

pub type ReservationCount = u16;
pub type JobIndex = usize;

#[derive(Debug)]
pub struct SocietyTaskReservations {
    reservations: Vec<(SocietyTask, Entity)>,
    /// Fast check for task membership in reservations
    task_membership: HashSet<SocietyTask>,
}

pub enum Reservation {
    Unreserved,
    ReservedBySelf,
    ReservedButShareable(ReservationCount),
    Unavailable,
}

impl Default for SocietyJobList {
    fn default() -> Self {
        Self {
            jobs: Vec::with_capacity(64),
            reservations: SocietyTaskReservations::default(),
            last_update: Tick::default(),
            no_more_jobs_temporarily: false,
        }
    }
}

impl SocietyJobList {
    pub fn submit<J: SocietyJobImpl + 'static>(&mut self, job: J) {
        self.submit_job(SocietyJob::create(job))
    }

    pub fn submit_job(&mut self, job: SocietyJobRef) {
        debug!("submitting society job"; "job" => ?job);
        assert!(
            !self.no_more_jobs_temporarily,
            "job indices are still held, or allow_jobs_again wasn't called"
        );
        self.jobs.push(job);
    }

    /// Returned job index is valid until [allow_jobs_again] is called
    pub fn filter_applicable_tasks(
        &mut self,
        entity: Entity,
        this_tick: Tick,
        world: &EcsWorld,
        tasks_out: &mut Vec<(SocietyTask, JobIndex, ReservationCount)>,
    ) {
        // refresh jobs if necessary
        if self.last_update != this_tick {
            self.last_update = this_tick;
            let len_before = self.jobs.len();
            trace!("refreshing {n} jobs", n = len_before);
            self.jobs.retain(|job| {
                let result = job.write().refresh_tasks(world);
                match result {
                    None => true,
                    Some(result) => {
                        debug!("job finished"; "result" => ?result, "job" => ?job);
                        false
                    }
                }
            });

            let len_after = self.jobs.len();
            if len_before != len_after {
                trace!("pruned {n} finished jobs", n = len_before - len_after);
            }
        }

        // reset when finished with tasks
        self.no_more_jobs_temporarily = true;

        for (i, job) in self.jobs.iter().enumerate() {
            let job = job.read();
            // TODO filter jobs for entity

            for task in job.tasks() {
                use Reservation::*;
                match self.reservations.check_for(task, entity) {
                    Unreserved | ReservedBySelf => {
                        // wonderful, this task is fully available
                        tasks_out.push((task.clone(), i, 0));
                    }
                    ReservedButShareable(n) => {
                        // this task is available but already reserved by others
                        tasks_out.push((task.clone(), i, n));
                    }
                    Unavailable => {
                        // not available
                    }
                }
            }
        }
    }

    #[inline]
    pub fn reservations_mut(&mut self) -> &mut SocietyTaskReservations {
        &mut self.reservations
    }

    pub fn allow_jobs_again(&mut self) {
        self.no_more_jobs_temporarily = false;
    }

    pub fn by_index(&self, idx: usize) -> Option<SocietyJobRef> {
        self.jobs.get(idx).cloned()
    }

    pub fn reservation(&self, entity: Entity) -> Option<&SocietyTask> {
        self.reservations
            .reservations
            .iter()
            .find_map(move |(task, e)| (*e == entity).as_some(task))
    }
}

impl Default for SocietyTaskReservations {
    fn default() -> Self {
        Self {
            reservations: Vec::with_capacity(128),
            task_membership: HashSet::with_capacity(128),
        }
    }
}

impl SocietyTaskReservations {
    /// Removes any other reservation for the given entity
    pub fn reserve(&mut self, task: SocietyTask, reserver: Entity) {
        self.task_membership.insert(task.clone());

        let tup = (task, reserver);

        if let Some(current_idx) = self.reservations.iter().position(|(_, e)| *e == reserver) {
            let mut existing = &mut self.reservations[current_idx];
            debug!("replaced existing reservation"; E(reserver), "new" => ?tup.0.clone(), "prev" => ?existing.0);

            debug_assert_ne!(existing, &tup, "duplicate reservation for {:?}", tup);

            existing.0 = tup.0;

            // ensure no other reservations
            debug_assert!(
                !self
                    .reservations
                    .iter()
                    .skip(current_idx)
                    .any(|(_, e)| *e == reserver),
                "{} has multiple reservations",
                E(reserver)
            );
        } else {
            debug!("adding new reservation"; E(reserver), "new" => ?tup.0.clone());
            self.reservations.push(tup);
        }
    }

    /// Removes current reservation, not necessary if immediately followed by a new reservation
    pub fn cancel(&mut self, reserver: Entity) {
        if let Some(current_idx) = self.reservations.iter().position(|(_, e)| *e == reserver) {
            let (prev, _) = self.reservations.swap_remove(current_idx);
            debug!("unreserved task"; E(reserver), "prev" => ?prev);

            // ensure no other reservations
            debug_assert!(
                !self
                    .reservations
                    .iter()
                    .skip(current_idx)
                    .any(|(_, e)| *e == reserver),
                "{} had multiple reservations",
                E(reserver)
            );
        } else {
            trace!("no task to unreserve"; E(reserver));
        }
    }

    pub fn check_for(&self, task: &SocietyTask, reserver: Entity) -> Reservation {
        // fast check
        if !self.task_membership.contains(task) {
            return Reservation::Unreserved;
        }

        let is_shareable = task.is_shareable();
        let mut count = 0;
        for (reserved_task, reserving_entity) in self.reservations.iter() {
            if task != reserved_task {
                continue;
            }

            if reserver == *reserving_entity {
                // itsa me
                return Reservation::ReservedBySelf;
            } else if !is_shareable {
                // someone else has reserved an unshareable task
                return Reservation::Unavailable;
            }

            // count shared reservations
            count += 1;
        }

        Reservation::ReservedButShareable(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        todo!()
    }
}
