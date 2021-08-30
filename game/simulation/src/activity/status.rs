//! Status channel for polling an active activity future

use std::cell::{Ref, RefCell};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::rc::Rc;

use common::*;

pub trait Status: Display {}
struct Inner(DynSlot<'static, dyn Display>);

pub struct StatusUpdater(Rc<RefCell<Inner>>);
pub struct StatusReceiver(Rc<RefCell<Inner>>);

pub struct StatusRef<'a> {
    guard: Ref<'a, Inner>,
}

#[derive(Copy, Clone)]
struct NopStatus;

pub fn status_channel() -> (StatusUpdater, StatusReceiver) {
    let inner = Inner(dynslot_new!(NopStatus));
    let inner = Rc::new(RefCell::new(inner));
    let tx = StatusUpdater(inner.clone());
    let rx = StatusReceiver(inner);
    (tx, rx)
}

impl<D: Display> Status for D {}

impl StatusUpdater {
    pub fn update<D: Display + 'static>(&self, state: D) {
        let mut status = self.0.borrow_mut();
        dynslot_update!(status.0, state);
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
    type Target = dyn Display;

    fn deref(&self) -> &Self::Target {
        self.guard.0.get()
    }
}

impl Display for NopStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Doing nothing")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_updater() {
        let (tx, rx) = status_channel();
        tx.update("nice");
        assert_eq!(format!("{}", &*rx.current()), "nice");

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

        tx.update(MyState::DoingWell);
        assert_eq!(format!("{}", &*rx.current()), "DoingWell");

        tx.update(MyState::ShatMyself);
        assert_eq!(format!("{}", &*rx.current()), "ShatMyself"); // oh dear
    }
}
