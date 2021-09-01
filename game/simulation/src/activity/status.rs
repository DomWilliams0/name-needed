//! Status channel for polling an active activity future

use std::cell::{Ref, RefCell};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::rc::Rc;

use common::*;

pub trait Status: Display {
    fn exertion(&self) -> f32;
}

struct Inner(DynSlot<'static, dyn Status>);

#[derive(Clone)]
pub struct StatusUpdater(Rc<RefCell<Inner>>);
pub struct StatusReceiver(Rc<RefCell<Inner>>);

pub struct StatusRef<'a> {
    guard: Ref<'a, Inner>,
}

#[derive(Copy, Clone)]
pub struct NopStatus;

pub fn status_channel() -> (StatusUpdater, StatusReceiver) {
    let inner = Inner(dynslot_new!(NopStatus));
    let inner = Rc::new(RefCell::new(inner));
    let tx = StatusUpdater(inner.clone());
    let rx = StatusReceiver(inner);
    (tx, rx)
}

impl StatusUpdater {
    pub fn update<S: Status + 'static>(&self, status: S) {
        let mut inner = self.0.borrow_mut();
        dynslot_update!(inner.0, status);
    }
}

impl StatusReceiver {
    pub fn current(&self) -> StatusRef {
        StatusRef {
            guard: self.0.borrow(),
        }
    }
}

impl Deref for StatusRef<'_> {
    type Target = dyn Status;

    fn deref(&self) -> &Self::Target {
        self.guard.0.get()
    }
}

impl Display for NopStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Doing nothing")
    }
}

impl Status for NopStatus {
    fn exertion(&self) -> f32 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MyStr(&'static str);

    impl Display for MyStr {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            Display::fmt(self.0, f)
        }
    }

    impl Status for MyStr {
        fn exertion(&self) -> f32 {
            1.5
        }
    }

    #[test]
    fn status_updater() {
        let (tx, rx) = status_channel();
        tx.update(MyStr("nice"));
        assert_eq!(format!("{}", &*rx.current()), "nice");
        assert!(rx.current().exertion().approx_eq(1.5, (f32::EPSILON, 2)));

        #[derive(Debug)]
        enum MyState {
            DoingWell,
            ShatMyself,
        }

        impl Display for MyState {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self)
            }
        }

        impl Status for MyState {
            fn exertion(&self) -> f32 {
                match self {
                    MyState::DoingWell => 1.0,
                    MyState::ShatMyself => 2.0,
                }
            }
        }

        tx.update(MyState::DoingWell);
        assert_eq!(format!("{}", &*rx.current()), "DoingWell");
        assert!(rx.current().exertion().approx_eq(1.0, (f32::EPSILON, 2)));

        tx.update(MyState::ShatMyself);
        assert_eq!(format!("{}", &*rx.current()), "ShatMyself"); // oh dear
        assert!(rx.current().exertion().approx_eq(2.0, (f32::EPSILON, 2)));
    }
}
