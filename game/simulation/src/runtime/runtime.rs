//! Async runtime for functionality crossing multiple ticks

use std::cell::RefCell;
use std::rc::Rc;
use std::task::Poll;

use cooked_waker::{IntoWaker, ViaRawPointer, Wake, WakeRef};
use futures::future::LocalBoxFuture;
use futures::prelude::*;
use futures::task::Context;

use common::*;

struct RuntimeInner {
    ready: Vec<TaskRef>,
    next_task: TaskHandle,
}

#[derive(Clone)]
pub struct Runtime(Rc<RefCell<RuntimeInner>>);

#[derive(Eq, PartialEq, Copy, Clone, Default)]
pub struct TaskHandle(u64);

struct Task {
    runtime: Runtime,
    handle: TaskHandle,
    future: RefCell<Option<LocalBoxFuture<'static, ()>>>,
}

#[derive(Clone)]
struct TaskRef(Rc<Task>);

// everything will run on the main thread
unsafe impl Send for TaskRef {}
unsafe impl Sync for TaskRef {}

impl Runtime {
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        let mut runtime = self.0.borrow_mut();
        let task = Task {
            runtime: self.clone(),
            handle: runtime.next_task_handle(),
            future: RefCell::new(Some(future.boxed_local())),
        };

        // task is ready immediately
        runtime.ready.push(TaskRef(Rc::new(task)));
    }

    /// Uses game state and events to update ready status of queued tasks, **does not execute any
    /// tasks**
    pub fn refresh_ready_tasks(&self) {
        // TODO
    }

    /// Polls all ready tasks
    pub fn tick(&self) {
        let mut runtime = self.0.borrow_mut();
        trace!("{} ready tasks", runtime.ready.len());
        for task in runtime.ready.drain(..) {
            let mut fut_slot = task.0.future.borrow_mut();
            if let Some(mut fut) = fut_slot.take() {
                // TODO unnecessary unconditional clone of task reference?
                let waker = task.clone().into_waker();
                let mut ctx = Context::from_waker(&waker);
                trace!("polling task"; "task" => ?task.0.handle);
                match fut.as_mut().poll(&mut ctx) {
                    Poll::Ready(_) => {
                        trace!("task is complete"; "task" => ?task.0.handle);
                    }
                    Poll::Pending => {
                        trace!("task is still ongoing"; "task" => ?task.0.handle);
                        *fut_slot = Some(fut);
                    }
                }
            }
        }
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
            next_task: TaskHandle::default(),
        });

        Runtime(Rc::new(inner))
    }
}

impl WakeRef for TaskRef {
    fn wake_by_ref(&self) {
        let mut runtime = self.0.runtime.0.borrow_mut();
        runtime.ready.push(self.clone());
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

impl crate::event::Token for TaskHandle {
    fn increment(&mut self) -> Self {
        let prev = *self;
        self.0 += 1;
        prev
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::runtime::ManualFuture;

    use super::*;
    use common::bumpalo::core_alloc::sync::Arc;

    #[test]
    fn basic_operation() {
        logging::for_tests();
        let runtime = Runtime::default();
        let fut = ManualFuture::default();
        let it_worked = Arc::new(AtomicBool::new(false));

        let fut2 = fut.clone();
        let it_worked2 = it_worked.clone();
        runtime.spawn(async move {
            debug!("here we go!!");
            let msg = fut2.await;
            debug!("all done!!!! string is '{}'", msg);
            it_worked2.store(true, Ordering::Relaxed);
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

        assert!(it_worked.load(Ordering::Relaxed), "future did not complete");
    }
}
