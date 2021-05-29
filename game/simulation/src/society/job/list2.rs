use crate::ecs::E;
use crate::job::job2::{SocietyJob, SocietyJobImpl};
use crate::job::SocietyTask;
use crate::simulation::Tick;
use crate::society::job::job2::SocietyJobRef;
use crate::{EcsWorld, Entity};
use common::*;
use std::collections::HashSet;

#[derive(Debug)]
pub struct SocietyJobList {
    jobs: Vec<SocietyJobRef>,
    reservations: SocietyTaskReservations,
    last_update: Tick,
}

pub type ReservationCount = u16;

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
        }
    }
}

impl SocietyJobList {
    pub fn submit<J: SocietyJobImpl + 'static>(&mut self, job: J) {
        self.submit_job(SocietyJob::create(job))
    }

    pub fn submit_job(&mut self, job: SocietyJobRef) {
        debug!("submitting society job"; "job" => ?job);
        self.jobs.push(job);
    }

    pub fn filter_applicable_tasks(
        &mut self,
        entity: Entity,
        this_tick: Tick,
        world: &EcsWorld,
        tasks_out: &mut Vec<(SocietyTask, ReservationCount)>,
    ) {
        // refresh jobs if necessary
        if self.last_update != this_tick {
            self.last_update = this_tick;
            trace!("refreshing {} jobs", self.jobs.len());
            for job in &self.jobs {
                let mut job = job.write();
                job.refresh_tasks(world);
            }
        }

        for job in &self.jobs {
            let job = job.read();
            // TODO filter jobs for entity

            for task in job.tasks() {
                use Reservation::*;
                match self.reservations.check_for(task, entity) {
                    Unreserved | ReservedBySelf => {
                        // wonderful, this task is fully available
                        tasks_out.push((task.clone(), 0));
                    }
                    ReservedButShareable(n) => {
                        // this task is available but already reserved by others
                        tasks_out.push((task.clone(), n));
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
