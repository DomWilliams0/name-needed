use common::parking_lot::Mutex;
use futures::task::Waker;
use futures::Future;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Default, Clone)]
pub struct ManualFuture<V> {
    // TODO could use rc and refcell instead
    state: Arc<Mutex<(bool, Option<Waker>, Option<V>)>>,
}

impl<V> Future for ManualFuture<V> {
    type Output = V;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();
        if state.0 {
            Poll::Ready(state.2.take().unwrap())
        } else {
            state.1 = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<V> ManualFuture<V> {
    pub fn trigger(&self, val: V) {
        let mut state = self.state.lock();
        state.0 = true;
        state.2 = Some(val);
        state
            .1
            .take()
            .expect("waker not set for triggered event")
            .wake();
    }
}
