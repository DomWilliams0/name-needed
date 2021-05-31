use crate::activity::ActivityFinish;
use crate::job::SocietyTask;
use crate::EcsWorld;

use common::parking_lot::RwLock;
use common::*;
use std::convert::TryFrom;
use std::ops::Deref;
use std::rc::Rc;

/// A high-level society job that produces a number of [SocietyTask]s
pub struct SocietyJob {
    /// Tasks still in progress
    tasks: Vec<SocietyTask>,

    pending_complete: Vec<(SocietyTask, SocietyTaskResult)>,

    // TODO remove box and make this type unsized, it's in an rc anyway
    inner: Box<dyn SocietyJobImpl>,
    // TODO weak references to other jobs that act as dependencies to this one, to enable/cancel them
}

#[derive(Debug)]
pub enum SocietyTaskResult {
    Success,
    Failure(Box<dyn Error>),
}

#[repr(transparent)]
#[derive(Clone)]
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

    pub(in crate::society::job) fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
    ) -> Option<SocietyTaskResult> {
        self.inner
            .refresh_tasks(world, &mut self.tasks, self.pending_complete.drain(..))
    }

    pub fn tasks(&self) -> impl Iterator<Item = &SocietyTask> + '_ {
        self.tasks.iter()
    }

    pub fn notify_completion(&mut self, task: SocietyTask, result: SocietyTaskResult) {
        self.pending_complete.push((task, result));
    }

    pub fn inner(&self) -> &dyn SocietyJobImpl {&*self.inner}
}

impl TryFrom<ActivityFinish> for SocietyTaskResult {
    type Error = ();

    fn try_from(finish: ActivityFinish) -> Result<Self, Self::Error> {
        match finish {
            ActivityFinish::Success => Ok(Self::Success),
            ActivityFinish::Failure(err) => Ok(Self::Failure(err)),
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

impl Deref for SocietyJobRef {
    type Target = Rc<RwLock<SocietyJob>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for SocietyJobRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.0.try_read() {
            None => write!(f, "<locked>)"),
            Some(job) => write!(f, "{} ({} tasks)", job.inner, job.tasks.len()),
        }
    }
}
