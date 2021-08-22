use crate::event::{RuntimeTimers, TimerToken};
use crate::{ComponentWorld, EcsWorld};
use common::*;
use futures::task::Waker;
use futures::Future;
use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

#[derive(Clone)]
pub struct ManualFuture<V>(Rc<RefCell<ManualFutureInner<V>>>);

#[derive(Copy, Clone)]
enum TriggerStatus {
    NotTriggered,
    Triggered,
    Cancelled,
}

struct ManualFutureInner<V> {
    state: TriggerStatus,
    waker: Option<Waker>,
    value: MaybeUninit<V>,
}

#[derive(Clone)]
pub struct TimerFuture<'w> {
    trigger: ManualFuture<()>,
    /// For cancelling on drop
    token: TimerToken,
    world: Pin<&'w EcsWorld>,
}

// only used on main thread
unsafe impl<V> Send for ManualFuture<V> {}
unsafe impl Send for TimerFuture<'_> {}

impl<V> Default for ManualFuture<V> {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(ManualFutureInner {
            state: TriggerStatus::NotTriggered,
            waker: None,
            value: MaybeUninit::uninit(),
        })))
    }
}

impl<V> Drop for ManualFutureInner<V> {
    fn drop(&mut self) {
        if matches!(self.state, TriggerStatus::Triggered) {
            // safety: value was initialised on trigger and not consumed
            unsafe { std::ptr::drop_in_place(self.value.as_mut_ptr()) }
        }
    }
}

impl<V> ManualFuture<V> {
    pub fn trigger(&self, val: V) {
        let mut inner = self.0.borrow_mut();
        inner.value = MaybeUninit::new(val);
        inner.state = TriggerStatus::Triggered;
        inner
            .waker
            .take()
            .expect("waker not set for triggered event")
            .wake();
    }

    fn state(&self) -> TriggerStatus {
        self.0.borrow().state
    }
}

impl<V> Future for ManualFuture<V> {
    type Output = V;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.0.borrow_mut();
        if let TriggerStatus::Triggered = inner.state {
            inner.state = TriggerStatus::Cancelled; // dont drop value again in destructor

            let val = std::mem::replace(&mut inner.value, MaybeUninit::uninit());

            // safety: value is initialised on trigger
            let val = unsafe { val.assume_init() };
            Poll::Ready(val)
        } else {
            inner.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<'w> TimerFuture<'w> {
    pub fn new(trigger: ManualFuture<()>, token: TimerToken, world: Pin<&'w EcsWorld>) -> Self {
        Self {
            trigger,
            token,
            world,
        }
    }
}

impl Drop for TimerFuture<'_> {
    fn drop(&mut self) {
        if matches!(self.trigger.state(), TriggerStatus::NotTriggered) {
            trace!(
                "cancelling timer {:?} due to task dropping before trigger",
                self.token
            );
            let timers = self.world.resource_mut::<RuntimeTimers>();
            if !timers.cancel(self.token) {
                warn!("failed to cancel timer {:?}", self.token);
            }
        }
    }
}

impl Future for TimerFuture<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut Pin::into_inner(self).trigger).poll(cx)
    }
}
