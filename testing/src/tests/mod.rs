// tests must be registered in lib::register_tests!

use crate::HookResult;
use std::any::Any;
use std::cell::RefCell;

pub mod dummy;

pub struct TestWrapper {
    test: RefCell<Box<dyn Any>>,
}

impl TestWrapper {
    pub fn new<T: 'static>(t: T) -> Self {
        Self {
            test: RefCell::new(Box::new(t)),
        }
    }

    pub fn invoke_with_self(&self, do_it: impl FnOnce(&mut ()) -> HookResult) -> crate::HookResult {
        let mut this = self.test.borrow_mut();
        let this = (&mut **this) as *mut _ as *mut ();
        unsafe { do_it(&mut *this) }
    }
}
