//! Async runtime for functionality crossing multiple ticks

use std::cell::RefCell;
use std::collections::VecDeque;
use std::hint::unreachable_unchecked;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;

use cooked_waker::{IntoWaker, ViaRawPointer, Wake, WakeRef};
use futures::channel::oneshot::Sender;
use futures::future::LocalBoxFuture;
use futures::prelude::*;
use futures::task::Context;

use common::*;

use crate::event::EntityEvent;
use crate::runtime::futures::ParkUntilWakeupFuture;

struct RuntimeInner {
    ready: Vec<WeakTaskRef>,
    /// Swapped out with `ready` during tick
    ready_double_buf: Vec<WeakTaskRef>,

    next_task: TaskHandle,

    /// Stores all triggered events for use by e2e tests
    #[cfg(feature = "testing")]
    event_log: Vec<crate::event::EntityEvent>,
}

#[derive(Clone)]
pub struct Runtime(Rc<RefCell<RuntimeInner>>);

#[derive(Eq, PartialEq, Copy, Clone, Default)]
pub struct TaskHandle(u64);

pub enum TaskFuture {
    Running(LocalBoxFuture<'static, BoxedResult<()>>),
    Polling,
    Done(TaskResult),
    DoneButConsumed,
}

#[derive(Debug)]
pub enum TaskResult {
    Cancelled,
    Finished(BoxedResult<()>),
}

pub struct Task {
    runtime: Runtime,
    handle: TaskHandle,
    future: RefCell<TaskFuture>,
    // TODO reuse/share/pool this allocation between tasks, maybe own it in the runtime
    event_sink: RefCell<VecDeque<EntityEvent>>,
    ready: AtomicBool,
}

impl Drop for Task {
    fn drop(&mut self) {
        trace!("dropping task {:?}", self.handle);
    }
}

#[derive(Clone, Debug)]
pub struct TaskRef(Rc<Task>);

pub struct WeakTaskRef(Weak<Task>);

// everything will run on the main thread
unsafe impl Send for TaskRef {}
unsafe impl Sync for TaskRef {}
unsafe impl Send for WeakTaskRef {}
unsafe impl Sync for WeakTaskRef {}

impl Runtime {
    pub fn spawn(
        &self,
        gimme_task_ref: Sender<TaskRef>,
        future: impl Future<Output = BoxedResult<()>> + 'static,
    ) -> TaskRef {
        let mut runtime = self.0.borrow_mut();
        let task = Task {
            runtime: self.clone(),
            handle: runtime.next_task_handle(),
            future: RefCell::new(TaskFuture::Running(future.boxed_local())),
            event_sink: RefCell::new(VecDeque::new()),
            ready: AtomicBool::new(false),
        };

        let task = TaskRef(Rc::new(task));

        // send task ref to future
        let _ = gimme_task_ref.send(task.clone());

        // task is ready immediately
        runtime.ready.push(task.weak());
        task.0.ready.store(true, Ordering::Relaxed);
        task
    }

    /// Polls all ready tasks
    pub fn tick(&self) {
        let mut runtime = self.0.borrow_mut();
        if !runtime.ready.is_empty() {
            trace!("{} ready tasks", runtime.ready.len());
        }

        // temporarily move ready tasks out of runtime so we can release the mutable ref
        let mut ready_tasks = {
            let to_consume = std::mem::take(&mut runtime.ready);
            // use cached double buf allocation for any tasks readied up during tick
            let runtime = &mut *runtime; // pls borrowck
            std::mem::swap(&mut runtime.ready, &mut runtime.ready_double_buf);
            debug_assert!(runtime.ready.is_empty());
            to_consume
        };

        drop(runtime);
        for task in ready_tasks.drain(..).filter_map(|t| t.upgrade()) {
            let was_ready = task.0.ready.swap(false, Ordering::Relaxed);
            debug_assert!(was_ready, "task should've been ready but wasn't");

            task.poll_task();
        }

        // swap ready list back
        let mut runtime = self.0.borrow_mut();
        let mut double_buf = std::mem::replace(&mut runtime.ready, ready_tasks);

        // move any newly ready tasks into proper ready queue out of double buf
        runtime.ready.append(&mut double_buf);

        // store double buf allocation again
        let dummy = std::mem::replace(&mut runtime.ready_double_buf, double_buf);
        debug_assert!(dummy.is_empty());
        std::mem::forget(dummy);
    }

    /// Can be called multiple times
    pub fn mark_ready(&self, task: &TaskRef) {
        trace!("marking task as ready"; "task" => ?task.handle());
        if !task.0.ready.swap(true, Ordering::Relaxed) {
            debug_assert!(
                !self.is_ready(task.handle()),
                "task handle ready flag is wrong, should be not ready"
            );
            self.0.borrow_mut().ready.push(task.weak());
        } else {
            debug_assert!(
                self.is_ready(task.handle()),
                "task handle ready flag is wrong, should be ready"
            );
        }
    }

    fn is_ready(&self, task: TaskHandle) -> bool {
        self.find_ready(task).is_some()
    }

    fn find_ready(&self, task: TaskHandle) -> Option<usize> {
        self.0
            .borrow()
            .ready
            .iter()
            .filter_map(|t| t.upgrade())
            .position(|t| t.handle() == task)
    }
}

#[cfg(feature = "testing")]
impl Runtime {
    pub fn post_events(&self, events: impl Iterator<Item = EntityEvent>) {
        let mut inner = self.0.borrow_mut();
        inner.event_log.extend(events);
    }

    /// Only used in tests, so allocation waste doesn't matter
    pub fn event_log(&self) -> Vec<EntityEvent> {
        let inner = self.0.borrow();
        inner.event_log.clone()
    }

    pub fn clear_event_log(&self) {
        let mut inner = self.0.borrow_mut();
        inner.event_log.clear();
    }
}

impl RuntimeInner {
    fn next_task_handle(&mut self) -> TaskHandle {
        let this = self.next_task;
        self.next_task.0 += 1;
        this
    }
}

impl Default for Runtime {
    fn default() -> Self {
        let inner = RefCell::new(RuntimeInner {
            ready: Vec::with_capacity(128),
            ready_double_buf: Vec::with_capacity(128),
            next_task: TaskHandle::default(),
            #[cfg(feature = "testing")]
            event_log: Vec::new(),
        });

        Runtime(Rc::new(inner))
    }
}

impl TaskRef {
    pub fn is_finished(&self) -> bool {
        let fut = self.0.future.borrow();
        match &*fut {
            TaskFuture::DoneButConsumed | TaskFuture::Done(_) => true,
            TaskFuture::Running(_) => false,
            TaskFuture::Polling => unreachable!(),
        }
    }

    /// Call once only, panics the second time
    pub fn result(&self) -> Option<TaskResult> {
        let mut fut = self.0.future.borrow_mut();
        match &*fut {
            TaskFuture::Running(_) => None,
            TaskFuture::Polling => unreachable!(),
            TaskFuture::DoneButConsumed => panic!("result has already been consumed"),
            TaskFuture::Done(_) => {
                let done = std::mem::replace(&mut *fut, TaskFuture::DoneButConsumed);
                let result = match done {
                    TaskFuture::Done(res) => res,
                    _ => unsafe { unreachable_unchecked() }, // already checked
                };
                Some(result)
            }
        }
    }

    pub fn is_ready(&self) -> bool {
        self.0.ready.load(Ordering::Relaxed)
    }

    /// Only wakes up when the runtime manually wakes it up via event
    pub async fn park_until_triggered(&self) {
        ParkUntilWakeupFuture::default().await
    }

    pub fn cancel(self) {
        let mut fut = self.0.future.borrow_mut();
        match &mut *fut {
            TaskFuture::Running(_) => {
                trace!("cancelling task {:?}", self.0.handle);
                // drop future
                *fut = TaskFuture::Done(TaskResult::Cancelled);
            }
            TaskFuture::Done(res) => {
                // consume
                debug!("cancelling finished task {:?}, consuming result", self.0.handle; "result" => ?res);
                *fut = TaskFuture::Done(TaskResult::Cancelled);
            }
            TaskFuture::Polling => unreachable!("task is in invalid state"),
            TaskFuture::DoneButConsumed => {
                drop(fut);
                warn!("cancelling task that's already consumed"; "task" => ?self.0);
            }
        }
    }

    fn poll_task(self) {
        let mut fut_slot = self.0.future.borrow_mut();

        // take ownership for poll
        let fut = std::mem::replace(&mut *fut_slot, TaskFuture::Polling);

        if let TaskFuture::Running(mut fut) = fut {
            // TODO reimplement raw waiter manually to avoid this unconditional clone
            let waker = self.clone().into_waker();
            let mut ctx = Context::from_waker(&waker);
            trace!("polling task"; "task" => ?self.0.handle);
            match fut.as_mut().poll(&mut ctx) {
                Poll::Ready(result) => {
                    trace!("task is complete"; "task" => ?self.0.handle, "result" => ?result);
                    *fut_slot = TaskFuture::Done(TaskResult::Finished(result));
                }
                Poll::Pending => {
                    trace!("task is still ongoing"; "task" => ?self.0.handle);
                    *fut_slot = TaskFuture::Running(fut);
                }
            }
        }
    }

    pub fn push_event(&self, event: EntityEvent) {
        self.0.event_sink.borrow_mut().push_back(event);
    }

    pub fn pop_event(&self) -> Option<EntityEvent> {
        self.0.event_sink.borrow_mut().pop_front()
    }

    pub fn handle(&self) -> TaskHandle {
        self.0.handle
    }

    pub fn weak(&self) -> WeakTaskRef {
        WeakTaskRef(Rc::downgrade(&self.0))
    }
}

impl WeakTaskRef {
    pub fn upgrade(&self) -> Option<TaskRef> {
        self.0.upgrade().map(TaskRef)
    }

    #[cfg(test)]
    pub fn dangling() -> Self {
        Self(Weak::default())
    }
}

impl WakeRef for TaskRef {
    fn wake_by_ref(&self) {
        self.0.runtime.mark_ready(self);
    }
}

impl Wake for TaskRef {}

unsafe impl ViaRawPointer for TaskRef {
    type Target = Task;

    fn into_raw(self) -> *mut Task {
        Rc::into_raw(self.0) as *mut Task
    }

    unsafe fn from_raw(ptr: *mut Task) -> Self {
        Self(Rc::from_raw(ptr as *const Task))
    }
}

impl Debug for TaskHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "TaskHandle({:#x})", self.0)
    }
}

impl Debug for TaskFuture {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            TaskFuture::Running(_) => "Running",
            TaskFuture::Polling => "Polling (invalid state)",
            TaskFuture::DoneButConsumed => "Done(consumed)",
            TaskFuture::Done(res) => {
                return write!(f, "Done({:?})", res);
            }
        })
    }
}

impl Debug for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Task")
            .field("handle", &self.handle)
            .field("events", &self.event_sink.borrow().len())
            .field("ready", &self.ready.load(Ordering::Relaxed))
            .field("state", &self.future.borrow())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use futures::channel::oneshot::channel;

    use common::bumpalo::core_alloc::sync::Arc;

    use crate::runtime::futures::manual::ManualFuture;

    use super::*;

    #[test]
    fn basic_operation() {
        logging::for_tests();
        let runtime = Runtime::default();
        let fut = ManualFuture::default();
        let it_worked = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();

        let fut2 = fut.clone();
        let it_worked2 = it_worked.clone();
        let task = runtime.spawn(tx, async move {
            let _taskref = rx.await.unwrap();

            debug!("here we go!!");
            let msg = fut2.await;
            debug!("all done!!!! string is '{}'", msg);
            it_worked2.store(true, Ordering::Relaxed);
            Ok(())
        });

        assert!(!task.is_finished());

        for _ in 0..4 {
            runtime.tick();
            assert!(!task.is_finished());
        }

        fut.trigger("nice");
        assert!(!task.is_finished());

        for _ in 0..2 {
            runtime.tick();
        }

        assert!(task.is_finished());
        assert!(it_worked.load(Ordering::Relaxed), "future did not complete");
    }
}
