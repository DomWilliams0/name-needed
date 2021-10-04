use crate::event::{RuntimeTimers, TimerToken};
use crate::{ComponentWorld, EcsWorld, Tick};
use common::*;
use futures::future::FusedFuture;
use futures::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct TimerFuture<'w> {
    end_tick: Tick,
    /// For cancelling on drop
    token: TimerToken,
    world: Pin<&'w EcsWorld>,
}

// only used on main thread
unsafe impl Send for TimerFuture<'_> {}

/// Task must be manually readied up by runtime
pub struct ParkUntilWakeupFuture(ParkState);

#[derive(Copy, Clone, Debug)]
enum ParkState {
    Unpolled,
    Parked,
    Complete,
}

impl Future for ParkUntilWakeupFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        match self.0 {
            ParkState::Unpolled => {
                // first call
                self.0 = ParkState::Parked;

                // intentionally does use waker - this will be done by the runtime
                Poll::Pending
            }
            ParkState::Parked => {
                // woken up
                self.0 = ParkState::Complete;
                Poll::Ready(())
            }
            ParkState::Complete => unreachable!("task has already been unparked"),
        }
    }
}

impl<'w> TimerFuture<'w> {
    pub fn new(end_tick: Tick, token: TimerToken, world: Pin<&'w EcsWorld>) -> Self {
        Self {
            token,
            end_tick,
            world,
        }
    }

    fn elapsed(&self) -> bool {
        let now = Tick::fetch();
        now.value() >= self.end_tick.value()
    }
}

impl Drop for TimerFuture<'_> {
    fn drop(&mut self) {
        if !self.elapsed() {
            trace!(
                "cancelling timer {:?} due to task dropping before trigger",
                self.token
            );
            let timers = self.world.resource_mut::<RuntimeTimers>();
            if !timers.cancel(self.token) {
                warn!("failed to cancel timer {:?} (already elapsed?)", self.token);
            }
        }
    }
}

impl Future for TimerFuture<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.elapsed() {
            cx.waker().wake_by_ref();
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl FusedFuture for ParkUntilWakeupFuture {
    fn is_terminated(&self) -> bool {
        // futures::future::Fuse will poll this once too many and mark as always Complete when it
        // isn't really. implementing this manually avoids this special case and removes extra
        // polls
        !matches!(self.0, ParkState::Unpolled)
    }
}

impl Default for ParkUntilWakeupFuture {
    fn default() -> Self {
        Self(ParkState::Unpolled)
    }
}

#[cfg(test)]
pub mod manual {
    use std::cell::RefCell;
    use std::future::Future;
    use std::mem::MaybeUninit;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    /// Beware, contains allocation
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

    // only used on main thread
    unsafe impl<V> Send for ManualFuture<V> {}

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
}
