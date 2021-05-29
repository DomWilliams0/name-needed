use crate::activity::ActivityFinish;
use crate::job::SocietyTask;
use crate::EcsWorld;
use common::derive_more::Deref;
use common::parking_lot::lock_api::RwLockReadGuard;
use common::parking_lot::{RawRwLock, RwLock};
use common::*;
use std::convert::TryFrom;
use std::rc::Rc;

/// A high-level society job that produces a number of [SocietyTask]s
pub struct SocietyJob {
    /// Tasks still in progress
    tasks: Vec<SocietyTask>,

    pending_complete: Vec<(SocietyTask, SocietyTaskResult)>,

    // TODO remove box and make this type unsized, it's in an rc anyway
    inner: Box<dyn SocietyJobImpl>,
}

#[derive(Debug)]
pub enum SocietyTaskResult {
    Success,
    Failure,
}

#[repr(transparent)]
#[derive(Clone, Deref)]
pub struct SocietyJobRef(Rc<RwLock<SocietyJob>>);

pub trait SocietyJobImpl: Display + Debug {
    /// [refresh] will be called after this before any tasks are dished out, so this can eagerly add
    /// tasks without filtering.
    ///
    /// TODO provide size hint that could be used as an optimisation for a small number of tasks (e.g. smallvec)
    fn populate_initial_tasks(&self, out: &mut Vec<SocietyTask>);

    /// Update `tasks` and apply `completions`.
    /// Return None if ongoing
    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: std::vec::Drain<(SocietyTask, SocietyTaskResult)>,
    ) -> Option<SocietyTaskResult>;
}

impl SocietyJob {
    pub fn create<J: SocietyJobImpl + 'static>(job: J) -> SocietyJobRef {
        let mut tasks = Vec::new();
        job.populate_initial_tasks(&mut tasks);

        SocietyJobRef(Rc::new(RwLock::new(SocietyJob {
            tasks,
            pending_complete: Vec::new(),
            inner: Box::new(job),
        })))
    }

    pub(in crate::society::job) fn refresh_tasks(&mut self, world: &EcsWorld) {
        self.inner
            .refresh_tasks(world, &mut self.tasks, self.pending_complete.drain(..));
    }

    pub fn tasks(&self) -> impl Iterator<Item = &SocietyTask> + '_ {
        self.tasks.iter()
    }
}

impl TryFrom<&ActivityFinish> for SocietyTaskResult {
    type Error = ();

    fn try_from(finish: &ActivityFinish) -> Result<Self, Self::Error> {
        match finish {
            ActivityFinish::Success => Ok(Self::Success),
            ActivityFinish::Failure(_) => Ok(Self::Failure),
            ActivityFinish::Interrupted => Err(()),
        }
    }
}

impl Debug for SocietyJobRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "SocietyJob(")?;

        match self.0.try_read() {
            None => write!(f, "<locked>)"),
            Some(job) => write!(
                f,
                "{:?} | {} tasks: {:?})",
                job.inner,
                job.tasks.len(),
                job.tasks
            ),
        }
    }
}
