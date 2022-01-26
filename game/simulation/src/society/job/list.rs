use std::collections::HashMap;
use std::num::NonZeroU16;

use common::*;

use crate::job::job::SocietyJobImpl;
use crate::job::{BuildThingJob, SocietyJob, SocietyTask};

use crate::society::job::job::SocietyJobRef;
use crate::{EcsWorld, Entity, Societies, SocietyHandle};

#[derive(Debug)] // TODO implement manually
pub struct SocietyJobList {
    jobs: Vec<SocietyJobRef>,
    to_cancel: Vec<SocietyJobHandle>,
    reservations: SocietyTaskReservations,
    next_handle: SocietyJobHandle,
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
    /// How many in total can work on this task
    fn max_reservers(&self) -> NonZeroU16;
}

#[cfg_attr(test, derive(Eq, PartialEq, Debug))]
pub enum Reservation {
    Unreserved,
    ReservedBySelf,
    ReservedButShareable(ReservationCount),
    Unavailable,
}

/// Unique job id per society
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct SocietyJobHandle {
    society: SocietyHandle,
    idx: u32,
}

impl SocietyJobHandle {
    pub fn society(self) -> SocietyHandle {
        self.society
    }

    pub fn resolve(self, societies: &Societies) -> Option<SocietyJobRef> {
        societies
            .society_by_handle(self.society)
            .and_then(|society| society.jobs().find_job(self))
    }

    pub fn resolve_and_cast<J: SocietyJobImpl + 'static, R>(
        self,
        societies: &Societies,
        do_this: impl FnOnce(&J) -> R,
    ) -> Option<R> {
        let job_ref = societies
            .society_by_handle(self.society)
            .and_then(|society| society.jobs().find_job(self))?;

        let job = job_ref.borrow();
        let casted = job.cast::<J>()?;

        Some(do_this(casted))
    }

    pub fn resolve_and_cast_mut<J: SocietyJobImpl + 'static, R>(
        self,
        societies: &Societies,
        do_this: impl FnOnce(&mut J) -> R,
    ) -> Option<R> {
        let job_ref = societies
            .society_by_handle(self.society)
            .and_then(|society| society.jobs().find_job(self))?;

        let mut job = job_ref.borrow_mut();
        let casted = job.cast_mut::<J>()?;

        Some(do_this(casted))
    }
}

impl Debug for SocietyJobHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SocietyJobHandle({:?}:{})", self.society, self.idx)
    }
}

impl SocietyJobList {
    pub(in crate::society) fn new(society: SocietyHandle) -> Self {
        Self {
            jobs: Vec::with_capacity(64),
            to_cancel: Vec::with_capacity(4),
            reservations: SocietyTaskReservations::default(),
            next_handle: SocietyJobHandle { society, idx: 0 },
        }
    }

    pub fn submit(&mut self, world: &EcsWorld, job: impl SocietyJobImpl + 'static) {
        let handle = {
            let this = self.next_handle;
            self.next_handle.idx += 1;
            this
        };

        let job = SocietyJob::create(world, handle, job);
        self.submit_internal(world, job)
    }

    /// Without generic parameter to reduce code size
    fn submit_internal(&mut self, world: &EcsWorld, job: SocietyJobRef) {
        debug!("submitting society job"; "job" => ?job);
        {
            // TODO special case for build job should be expanded to all jobs needing progress tracking
            let j = job.borrow();
            if let Some(build) = j.cast::<BuildThingJob>() {
                world
                    .helpers_building()
                    .create_build(build.details(), job.handle());
            }
        }

        self.jobs.push(job);
    }

    pub fn cancel(&mut self, job: SocietyJobHandle) {
        if job.society == self.this_society() {
            self.to_cancel.push(job);
        }
    }

    /// Update once per tick to remove finished and cancelled jobs
    pub fn refresh_jobs(&mut self, world: &EcsWorld) {
        let len_before = self.jobs.len();
        trace!("refreshing {n} jobs", n = len_before);
        self.jobs.retain(|job| {
            let result = job.borrow_mut().refresh_tasks(world);
            match result {
                None => true,
                Some(result) => {
                    debug!("job finished"; "result" => ?result, "job" => ?job);
                    false
                }
            }
        });

        // remove cancelled jobs
        for handle in self.to_cancel.drain(..) {
            if let Some(idx) = self.jobs.iter().position(|j| j.handle() == handle) {
                self.jobs.swap_remove(idx);
                debug!("cancelled job"; "job" => ?handle);
            }
        }

        let len_after = self.jobs.len();
        if len_before != len_after {
            trace!(
                "pruned {n} finished and cancelled jobs",
                n = len_before - len_after
            );
        }
    }

    /// Returned job index is valid until [allow_jobs_again] is called
    pub fn filter_applicable_tasks(
        &self,
        entity: Entity,
        mut add_task: impl FnMut(SocietyTask, JobIndex, ReservationCount),
    ) {
        for (i, job) in self.jobs.iter().enumerate() {
            let job = job.borrow();
            // TODO filter jobs for entity

            for task in job.tasks() {
                use Reservation::*;
                let reservations = match self.reservations.check_for(task, entity) {
                    Unreserved | ReservedBySelf => {
                        // wonderful, this task is fully available
                        0
                    }
                    ReservedButShareable(n) => {
                        // this task is available but already reserved by others
                        n
                    }
                    Unavailable => {
                        // not available
                        continue;
                    }
                };
                add_task(task.clone(), i, reservations);
            }
        }
    }

    #[inline]
    pub fn reservations_mut(&mut self) -> &mut SocietyTaskReservations {
        &mut self.reservations
    }

    // TODO use SocietyJobHandle instead of indices
    pub fn by_index(&self, idx: usize) -> Option<SocietyJobRef> {
        self.jobs.get(idx).cloned()
    }

    pub fn find_job(&self, handle: SocietyJobHandle) -> Option<SocietyJobRef> {
        if handle.society == self.this_society() {
            self.jobs
                .iter()
                .find(|j| j.handle().idx == handle.idx)
                .cloned()
        } else {
            None
        }
    }

    const fn this_society(&self) -> SocietyHandle {
        self.next_handle.society
    }

    pub fn reservation(&self, entity: Entity) -> Option<&SocietyTask> {
        self.reservations
            .reservations
            .iter()
            .find_map(move |(task, e)| (*e == entity).as_some(task))
    }

    /// Quadratic complexity (n total tasks * m total reserved tasks)
    pub fn iter_all_filtered(
        &self,
        mut job_filter: impl FnMut(&SocietyJobRef) -> bool,
        mut per_task: impl FnMut(&SocietyTask, &SmallVec<[Entity; 3]>),
    ) {
        let mut reservers = SmallVec::new();
        for job in self.jobs.iter() {
            if !job_filter(job) {
                continue;
            }

            let job = job.borrow();
            for task in job.tasks() {
                // gather all reservations for this task, giving us some beautiful n^2 complexity
                self.reservations.check_all(task, |e| reservers.push(e));

                per_task(task, &reservers);
                reservers.clear();
            }
        }
    }

    pub fn iter_all(&self) -> impl Iterator<Item = &SocietyJobRef> + '_ {
        self.jobs.iter()
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
                debug!("reserving the same task again, doing nothing"; reserver, "task" => ?existing.0);
                return;
            }

            debug!("replaced existing reservation"; reserver, "new" => ?tup.0.clone(), "prev" => ?existing.0);

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
                reserver
            );
        } else {
            debug!("adding new reservation"; reserver, "new" => ?tup.0.clone());
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

    /// Removes current reservation, not necessary if immediately followed by a new reservation.
    /// TODO cancelling a shared reservation might cancel it for everyone else too, oh no
    pub fn cancel(&mut self, reserver: Entity) {
        if let Some(current_idx) = self.reservations.iter().position(|(_, e)| *e == reserver) {
            let (prev, _) = self.reservations.swap_remove(current_idx);
            self.release_ref(&prev);
            debug!("unreserved task"; reserver, "prev" => ?prev);

            // ensure no other reservations
            debug_assert!(
                !self
                    .reservations
                    .iter()
                    .skip(current_idx)
                    .any(|(_, e)| *e == reserver),
                "{} had multiple reservations",
                reserver
            );
        } else {
            trace!("no task to unreserve"; reserver);
        }
    }

    pub fn check_for(&self, task: &T, reserver: Entity) -> Reservation {
        // fast check
        let reserve_count = match self.task_membership.get(task) {
            None => return Reservation::Unreserved,
            Some(n) => *n,
        };

        let max_workers = task.max_reservers().get();
        let mut count = 0;
        for (reserved_task, reserving_entity) in self.reservations.iter() {
            if task != reserved_task {
                continue;
            }

            if reserver == *reserving_entity {
                // itsa me
                return Reservation::ReservedBySelf;
            } else if max_workers == 1 {
                // someone else has reserved an unshareable task
                return Reservation::Unavailable;
            }

            // count shared reservations
            count += 1;

            if count >= max_workers {
                // fully reserved
                return Reservation::Unavailable;
            }
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
    fn max_reservers(&self) -> NonZeroU16 {
        SocietyTask::max_workers(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::ecs::Builder;
    use crate::ComponentWorld;

    use super::*;

    impl ReservationTask for i32 {
        fn max_reservers(&self) -> NonZeroU16 {
            let n = if *self % 2 == 0 { 3 } else { 1 };

            NonZeroU16::new(n).unwrap()
        }
    }

    fn create() -> (Reservations<i32>, [Entity; 4]) {
        let reservations = Reservations::default();
        let ecs = EcsWorld::new();
        let entities = [
            ecs.create_entity().build().into(),
            ecs.create_entity().build().into(),
            ecs.create_entity().build().into(),
            ecs.create_entity().build().into(),
        ];
        (reservations, entities)
    }

    #[test]
    fn simple_unshareable() {
        let (mut reservations, [a, b, _, _]) = create();
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
        let (mut reservations, [a, b, c, d]) = create();
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

        reservations.reserve(task, c);
        assert_eq!(
            reservations.check_for(&task, c),
            Reservation::ReservedBySelf
        );
        assert_eq!(
            reservations.check_for(&task, d),
            Reservation::Unavailable // hit limit of 3 max workers
        );
    }

    #[test]
    fn replace_and_cancel() {
        let (mut reservations, [a, b, c, _]) = create();
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
        let (mut reservations, [a, b, c, _]) = create();
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
