use crate::ecs::E;
use crate::job::SocietyTask;
use crate::simulation::Tick;
use crate::society::job::job::SocietyJobRef;
use crate::{EcsWorld, Entity};
use common::*;
use std::collections::HashMap;

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
pub type SocietyTaskReservations = Reservations<SocietyTask>;

#[derive(Debug)]
pub struct Reservations<T> {
    reservations: Vec<(T, Entity)>,
    /// Fast check for membership in reservations
    task_membership: HashMap<T, ReservationCount>,
}

pub trait ReservationTask: Clone + Hash + Eq + Debug {
    fn is_shareable(&self) -> bool;
}

#[cfg_attr(test, derive(Eq, PartialEq, Debug))]
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
    pub fn submit(&mut self, job: SocietyJobRef) {
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

    pub(crate) fn allow_jobs_again(&mut self) {
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

    /// Quadratic complexity (n total tasks * m total reserved tasks)
    pub fn iter_all(
        &self,
        mut per_job: impl FnMut(&SocietyJobRef) -> bool,
        mut per_task: impl FnMut(&SocietyTask, &SmallVec<[Entity; 3]>),
    ) {
        let mut reservers = SmallVec::new();
        for job in self.jobs.iter() {
            if !per_job(job) {
                continue;
            }

            let job = job.read();
            for task in job.tasks() {
                // gather all reservations for this task, giving us some beautiful n^2 complexity
                self.reservations.check_all(task, |e| reservers.push(e));

                per_task(task, &reservers);
                reservers.clear();
            }
        }
    }
}

impl<T> Default for Reservations<T> {
    fn default() -> Self {
        Self {
            reservations: Vec::with_capacity(128),
            task_membership: HashMap::with_capacity(128),
        }
    }
}

impl<T: ReservationTask> Reservations<T> {
    /// Removes any other reservation for the given entity
    pub fn reserve(&mut self, task: T, reserver: Entity) {
        let tup = (task, reserver);
        if let Some(current_idx) = self.reservations.iter().position(|(_, e)| *e == reserver) {
            let existing = &mut self.reservations[current_idx];

            if existing.0 == tup.0 {
                // same task
                debug!("reserving the same task again, doing nothing"; E(reserver), "task" => ?existing.0);
                return;
            }

            debug!("replaced existing reservation"; E(reserver), "new" => ?tup.0.clone(), "prev" => ?existing.0);

            debug_assert_ne!(existing, &tup, "duplicate reservation for {:?}", tup);

            let prev = std::mem::replace(&mut existing.0, tup.0.clone());

            self.add_ref(tup.0);
            self.release_ref(&prev);

            // ensure no other reservations
            debug_assert!(
                !self
                    .reservations
                    .iter()
                    .skip(current_idx + 1)
                    .any(|(_, e)| *e == reserver),
                "{} has multiple reservations",
                E(reserver)
            );
        } else {
            debug!("adding new reservation"; E(reserver), "new" => ?tup.0.clone());
            self.add_ref(tup.0.clone());
            self.reservations.push(tup);
        }
    }

    fn add_ref(&mut self, task: T) {
        *self.task_membership.entry(task).or_default() += 1;
    }

    fn release_ref(&mut self, task: &T) {
        let count = self
            .task_membership
            .get_mut(task)
            .expect("missing reservation");
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.task_membership.remove(task);
        }
    }

    /// Removes current reservation, not necessary if immediately followed by a new reservation
    pub fn cancel(&mut self, reserver: Entity) {
        if let Some(current_idx) = self.reservations.iter().position(|(_, e)| *e == reserver) {
            let (prev, _) = self.reservations.swap_remove(current_idx);
            self.release_ref(&prev);
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

    pub fn check_for(&self, task: &T, reserver: Entity) -> Reservation {
        // fast check
        let reserve_count = match self.task_membership.get(task) {
            None => return Reservation::Unreserved,
            Some(n) => *n,
        };

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

        debug_assert_ne!(count, 0);
        debug_assert_eq!(count, reserve_count);
        Reservation::ReservedButShareable(count)
    }

    pub fn check_all(&self, task: &T, mut per_reserver: impl FnMut(Entity)) {
        // fast check
        let reserve_count = match self.task_membership.get(task) {
            None => return,
            Some(n) => *n,
        };

        let mut left = reserve_count;
        let mut reservations = self.reservations.iter();
        while left != 0 {
            if let Some((reserved_task, reserver)) = reservations.next() {
                if reserved_task == task {
                    per_reserver(*reserver);
                    left -= 1;
                }
            } else {
                break;
            }
        }
    }
}

impl ReservationTask for SocietyTask {
    fn is_shareable(&self) -> bool {
        SocietyTask::is_shareable(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::Builder;
    use crate::ComponentWorld;

    impl ReservationTask for i32 {
        fn is_shareable(&self) -> bool {
            *self % 2 == 0
        }
    }

    fn create() -> (Reservations<i32>, [Entity; 3]) {
        let reservations = Reservations::default();
        let ecs = EcsWorld::new();
        let entities = [
            ecs.create_entity().build(),
            ecs.create_entity().build(),
            ecs.create_entity().build(),
        ];
        (reservations, entities)
    }

    #[test]
    fn simple_unshareable() {
        let (mut reservations, [a, b, _]) = create();
        let task = 1; // odd = unshareable

        assert_eq!(reservations.check_for(&task, a), Reservation::Unreserved);
        assert_eq!(reservations.check_for(&task, b), Reservation::Unreserved);

        // reserve exclusively
        reservations.reserve(task, a);
        assert_eq!(
            reservations.check_for(&task, a),
            Reservation::ReservedBySelf
        );
        assert_eq!(reservations.check_for(&task, b), Reservation::Unavailable);
    }

    #[test]
    fn simple_shareable() {
        let (mut reservations, [a, b, c]) = create();
        let task = 2; // even = shareable

        // reserve but shared
        reservations.reserve(task, a);
        assert_eq!(
            reservations.check_for(&task, a),
            Reservation::ReservedBySelf
        );
        assert_eq!(
            reservations.check_for(&task, b),
            Reservation::ReservedButShareable(1)
        );
        assert_eq!(
            reservations.check_for(&task, c),
            Reservation::ReservedButShareable(1)
        );

        reservations.reserve(task, b);
        assert_eq!(
            reservations.check_for(&task, a),
            Reservation::ReservedBySelf
        );
        assert_eq!(
            reservations.check_for(&task, b),
            Reservation::ReservedBySelf
        );
        assert_eq!(
            reservations.check_for(&task, c),
            Reservation::ReservedButShareable(2)
        );
    }

    #[test]
    fn replace_and_cancel() {
        let (mut reservations, [a, b, c]) = create();
        let task = 9; // odd = unshareable

        // reserve other then swap
        reservations.reserve(13, a);
        reservations.reserve(task, a);

        assert_eq!(
            reservations.check_for(&task, a),
            Reservation::ReservedBySelf
        );
        assert_eq!(reservations.check_for(&task, b), Reservation::Unavailable);
        assert_eq!(reservations.check_for(&task, c), Reservation::Unavailable);

        reservations.cancel(a);

        assert_eq!(reservations.check_for(&task, a), Reservation::Unreserved);
        assert_eq!(reservations.check_for(&task, b), Reservation::Unreserved);
        assert_eq!(reservations.check_for(&task, c), Reservation::Unreserved);
    }

    #[test]
    fn check_all() {
        let (mut reservations, [a, b, c]) = create();
        let task = 4;

        macro_rules! collect_all {
            () => {{
                let mut v = vec![];
                reservations.check_all(&task, |e| v.push(e));
                v.sort();
                v
            }};
        }

        assert_eq!(collect_all!(), vec![]);

        // extra random tasks
        reservations.reserve(5, a);
        reservations.reserve(6, b);
        reservations.reserve(9, c);

        reservations.reserve(task, a);
        assert_eq!(collect_all!(), vec![a]);

        reservations.reserve(task, b);
        assert_eq!(collect_all!(), vec![a, b]);

        reservations.cancel(a);
        assert_eq!(collect_all!(), vec![b]);

        reservations.reserve(task, c);
        assert_eq!(collect_all!(), vec![b, c]);

        reservations.reserve(task, c); // dupe
        assert_eq!(collect_all!(), vec![b, c]);
    }
}
