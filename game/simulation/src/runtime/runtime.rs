//! Async runtime for functionality crossing multiple ticks

use std::cell::RefCell;
use std::rc::Rc;
use std::task::Poll;

use cooked_waker::{IntoWaker, ViaRawPointer, Wake, WakeRef};
use futures::future::LocalBoxFuture;
use futures::prelude::*;
use futures::task::Context;

use crate::activity::{ActivityComponent2, EventUnsubscribeResult};
use crate::ecs::WriteStorage;
use crate::event::{EntityEvent, EntityEventQueue, RuntimeTimers};
use crate::{Entity, Tick};
use common::*;
use futures::channel::oneshot::Sender;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

struct RuntimeInner {
    ready: Vec<TaskRef>,
    /// Swapped out with `ready` during tick
    ready_double_buf: Vec<TaskRef>,

    next_task: TaskHandle,
}

#[derive(Clone)]
pub struct Runtime(Rc<RefCell<RuntimeInner>>);

#[derive(Eq, PartialEq, Copy, Clone, Default)]
pub struct TaskHandle(u64);

pub struct Task {
    runtime: Runtime,
    handle: TaskHandle,
    future: RefCell<Option<LocalBoxFuture<'static, ()>>>,
    // TODO reuse/share/pool this allocation between tasks, maybe own it in the runtime
    event_sink: RefCell<VecDeque<EntityEvent>>,
    ready: AtomicBool,
}

impl Drop for Task {
    fn drop(&mut self) {
        trace!("dropping task {:?}", self.handle);
    }
}

#[derive(Clone)]
pub struct TaskRef(Rc<Task>);

// everything will run on the main thread
unsafe impl Send for TaskRef {}
unsafe impl Sync for TaskRef {}

impl Runtime {
    pub fn spawn(
        &self,
        gimme_task_ref: Sender<TaskRef>,
        future: impl Future<Output = ()> + 'static,
    ) -> TaskRef {
        let mut runtime = self.0.borrow_mut();
        let task = Task {
            runtime: self.clone(),
            handle: runtime.next_task_handle(),
            future: RefCell::new(Some(future.boxed_local())),
            event_sink: RefCell::new(VecDeque::new()),
            ready: AtomicBool::new(false),
        };

        let task = TaskRef(Rc::new(task));

        // send task ref to future
        let _ = gimme_task_ref.send(task.clone());

        // task is ready immediately
        runtime.ready.push(task.clone());
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
        for task in ready_tasks.drain(..) {
            let was_ready = task.0.ready.swap(false, Ordering::Relaxed);
            debug_assert!(was_ready, "task should've been ready but wasn't");

            task.poll();
        }

        // swap ready list back
        let mut runtime = self.0.borrow_mut();
        let mut double_buf = std::mem::replace(&mut runtime.ready, ready_tasks);

        // move any newly ready tasks into proper ready queue out of double buf
        runtime.ready.extend(double_buf.drain(..));

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
            self.0.borrow_mut().ready.push(task.clone());
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
            .position(|t| t.handle() == task)
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
        });

        Runtime(Rc::new(inner))
    }
}

impl TaskRef {
    pub fn is_finished(&self) -> bool {
        self.0.future.borrow().is_none()
    }

    pub fn is_ready(&self) -> bool {
        self.0.ready.load(Ordering::Relaxed)
    }

    /// Only wakes up when next event arrives for this task
    pub async fn park_until_event(&self) {
        pub struct ParkUntilEvent(bool);

        impl Future for ParkUntilEvent {
            type Output = ();

            fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
                if std::mem::replace(&mut self.0, true) {
                    Poll::Ready(())
                } else {
                    // intentionally does not wake up - this will be done when an event arrives
                    Poll::Pending
                }
            }
        }

        ParkUntilEvent(false).await
    }

    pub fn cancel(self) {
        trace!("cancelling task {:?}", self.0.handle);

        // drop future
        let _ = self.0.future.borrow_mut().take();
    }

    pub fn poll(&self) {
        let mut fut_slot = self.0.future.borrow_mut();
        if let Some(mut fut) = fut_slot.take() {
            // TODO unnecessary unconditional clone of task reference?
            let waker = self.clone().into_waker();
            let mut ctx = Context::from_waker(&waker);
            trace!("polling task"; "task" => ?self.0.handle);
            match fut.as_mut().poll(&mut ctx) {
                Poll::Ready(_) => {
                    trace!("task is complete"; "task" => ?self.0.handle);
                }
                Poll::Pending => {
                    trace!("task is still ongoing"; "task" => ?self.0.handle);
                    *fut_slot = Some(fut);
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::runtime::ManualFuture;

    use super::*;
    use common::bumpalo::core_alloc::sync::Arc;
    use futures::channel::oneshot::channel;

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
