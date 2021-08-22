//! Async runtime for functionality crossing multiple ticks

use futures::prelude::*;

use common::parking_lot::Mutex;
use common::*;
use futures::future::{BoxFuture, LocalBoxFuture};
use futures::task::waker_ref;
use futures::task::{ArcWake, Context};
use std::cell::RefCell;
use std::sync::Arc;
use std::task::Poll;

pub struct Runtime {
    ready: Vec<Arc<Task>>,
    next_task: TaskHandle,
}

// TODO could use Rc
#[derive(Clone)]
pub struct RuntimeHandle(Arc<RefCell<Runtime>>);

#[derive(Eq, PartialEq, Copy, Clone, Default)]
pub struct TaskHandle(u64);

pub struct Task {
    runtime: RuntimeHandle,
    handle: TaskHandle,
    // TODO dont need Send requirement
    // TODO dont need mutex or option probably
    future: Mutex<Option<BoxFuture<'static, ()>>>,
}

// idk
unsafe impl Sync for Task {}
unsafe impl Send for Task {}

impl Runtime {
    pub fn new() -> RuntimeHandle {
        let runtime = RefCell::new(Self {
            ready: Vec::with_capacity(128),
            next_task: TaskHandle::default(),
        });

        RuntimeHandle(Arc::new(runtime))
    }

    fn next_task_handle(&mut self) -> TaskHandle {
        let this = self.next_task;
        self.next_task.0 += 1;
        this
    }
}

impl RuntimeHandle {
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let mut runtime = self.0.borrow_mut();
        let task = Task {
            runtime: self.clone(),
            handle: runtime.next_task_handle(),
            future: Mutex::new(Some(future.boxed())),
        };

        // task is ready immediately
        runtime.ready.push(Arc::new(task));
    }

    /// Uses game state and events to update ready status of queued tasks, **does not execute any
    /// tasks**
    pub fn refresh_ready_tasks(&self) {}

    /// Polls all ready tasks
    pub fn tick(&self) {
        let mut runtime = self.0.borrow_mut();
        trace!("{} ready tasks", runtime.ready.len());
        for task in runtime.ready.drain(..) {
            // take temporarily TODO maybeuninit
            let mut fut_slot = task.future.lock();
            if let Some(mut fut) = fut_slot.take() {
                let waker = waker_ref(&task);
                let mut ctx = Context::from_waker(&*waker);
                trace!("polling task");
                match fut.as_mut().poll(&mut ctx) {
                    Poll::Ready(_) => {
                        trace!("task is complete"; "task" => ?task.handle);
                    }
                    Poll::Pending => {
                        trace!("task is still ongoing"; "task" => ?task.handle);
                        *fut_slot = Some(fut);
                    }
                }
            }
        }
    }
}

impl Default for RuntimeHandle {
    fn default() -> Self {
        unreachable!("insert resource manually!")
    }
}

// TODO dont need arc
impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let mut runtime = arc_self.runtime.0.borrow_mut();
        runtime.ready.push(arc_self.clone());
    }
}

impl Debug for TaskHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "TaskHandle({:#x})", self.0)
    }
}

impl crate::event::Token for TaskHandle {
    fn increment(&mut self) -> Self {
        let prev = *self;
        self.0 += 1;
        prev
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ManualFuture;

    #[test]
    fn basic_operation() {
        logging::for_tests();
        let runtime = Runtime::new();
        let fut = ManualFuture::default();
        let fut2 = fut.clone();
        runtime.spawn(async {
            debug!("here we go!!");
            let msg = fut2.await;
            debug!("all done!!!! string is '{}'", msg);
        });

        for _ in 0..4 {
            runtime.refresh_ready_tasks();
            runtime.tick();
        }

        fut.trigger("nice");

        for _ in 0..2 {
            runtime.refresh_ready_tasks();
            runtime.tick();
        }
    }
}
