use crate::job::SocietyTask;
use crate::{EcsWorld, Entity, WorldPosition, WorldPositionRange};

use common::parking_lot::RwLock;
use common::*;

use std::ops::Deref;
use std::rc::Rc;
use unit::world::WorldPoint;

/// A high-level society job that produces a number of [SocietyTask]s. Unsized but it lives in an
/// Rc anyway
pub struct SocietyJob<J: ?Sized = dyn SocietyJobImpl> {
    /// Tasks still in progress
    tasks: Vec<SocietyTask>,

    pending_complete: SmallVec<[(SocietyTask, SocietyTaskResult); 1]>,

    inner: J,
}

#[derive(Debug)]
pub enum SocietyTaskResult {
    Success,
    Failure(Box<dyn Error>),
}

#[repr(transparent)]
#[derive(Clone)]
pub struct SocietyJobRef(Rc<RwLock<SocietyJob>>);

pub(in crate::society::job) type CompletedTasks<'a> = &'a mut [(SocietyTask, SocietyTaskResult)];

pub trait SocietyJobImpl: Display + Debug {
    /// [refresh_tasks] will be called after this before any tasks are dished out, so this can eagerly add
    /// tasks without filtering.
    ///
    /// TODO provide size hint that could be used as an optimisation for a small number of tasks (e.g. smallvec)
    fn populate_initial_tasks(&self, world: &EcsWorld, out: &mut Vec<SocietyTask>);

    /// Update `tasks` and apply `completions`. Completions are considered owned by this method, as
    /// the underlying container will be cleared on return, so feel free to move results out.
    /// Return None if ongoing.
    /// If 0 tasks are left this counts as a success.
    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: CompletedTasks,
    ) -> Option<SocietyTaskResult>;
}

/// Declarative society command that maps to a [SocietyJob]
#[derive(Debug)]
pub enum SocietyCommand {
    BreakBlocks(WorldPositionRange),
    HaulToPosition(Entity, WorldPoint),

    /// (thing, container)
    HaulIntoContainer(Entity, Entity),
}

impl SocietyCommand {
    pub fn into_job(self, world: &EcsWorld) -> Result<SocietyJobRef, Self> {
        use self::SocietyCommand::*;
        use crate::job::jobs::*;

        macro_rules! job {
            ($job:expr) => {
                Ok(SocietyJob::create(world, $job))
            };
        }

        // TODO return a dyn error in result
        match self {
            BreakBlocks(range) => job!(BreakBlocksJob::new(range)),
            HaulToPosition(e, pos) => {
                job!(HaulJob::with_target_position(e, pos, world).ok_or(self)?)
            }
            HaulIntoContainer(e, container) => {
                job!(HaulJob::with_target_container(e, container, world).ok_or(self)?)
            }
        }
    }
}

impl SocietyJob<dyn SocietyJobImpl> {
    pub fn create(world: &EcsWorld, job: impl SocietyJobImpl + 'static) -> SocietyJobRef {
        let mut tasks = Vec::new();
        job.populate_initial_tasks(world, &mut tasks);

        SocietyJobRef(Rc::new(RwLock::new(SocietyJob {
            tasks,
            pending_complete: SmallVec::new(),
            inner: job,
        })))
    }

    pub(in crate::society::job) fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
    ) -> Option<SocietyTaskResult> {
        let ret = self
            .inner
            .refresh_tasks(world, &mut self.tasks, &mut self.pending_complete)
            .or_else(|| {
                if self.tasks.is_empty() {
                    // no tasks left and no specific result returned
                    Some(SocietyTaskResult::Success)
                } else {
                    None
                }
            });

        self.pending_complete.clear();
        ret
    }

    pub fn tasks(&self) -> impl Iterator<Item = &SocietyTask> + '_ {
        self.tasks.iter()
    }

    pub fn notify_completion(&mut self, task: SocietyTask, result: SocietyTaskResult) {
        self.pending_complete.push((task, result));
    }

    pub fn inner(&self) -> &dyn SocietyJobImpl {
        &self.inner
    }
}

impl From<BoxedResult<()>> for SocietyTaskResult {
    fn from(result: BoxedResult<()>) -> Self {
        match result {
            Ok(_) => Self::Success,
            Err(err) => Self::Failure(err),
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
                &job.inner,
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
            Some(job) => write!(f, "{} ({} tasks)", &job.inner, job.tasks.len()),
        }
    }
}
