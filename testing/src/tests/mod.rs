// tests must be registered in lib::register_tests!

use crate::HookResult;
use std::any::Any;
use std::cell::RefCell;

// ---------- test modules
pub mod dummy;
pub mod equip_with_pickup;
// ---------- end test modules

pub struct TestWrapper {
    test: RefCell<Box<dyn Any>>,
    state: RefCell<TestState>,
}

#[derive(Default)]
struct TestState {
    /// Next tick to call on_tick
    wait_until: Option<u32>,
}

pub struct TestHelper<'a>(&'a RefCell<TestState>);

impl TestWrapper {
    pub fn new<T: 'static>(t: T) -> Self {
        Self {
            test: RefCell::new(Box::new(t)),
            state: RefCell::new(TestState::default()),
        }
    }

    pub fn invoke_with_self(&self, do_it: impl FnOnce(&mut ()) -> HookResult) -> HookResult {
        if let Some(next) = self.state.borrow().wait_until {
            let current = simulation::Tick::fetch();
            if current.value() < next {
                return HookResult::KeepGoing;
            }
        }

        let mut this = self.test.borrow_mut();
        let this = (&mut **this) as *mut _ as *mut ();
        unsafe { do_it(&mut *this) }
    }

    pub fn helper(&self) -> TestHelper {
        TestHelper(&self.state)
    }
}

impl TestHelper<'_> {
    pub fn wait_n_ticks(&self, n: u32) {
        let now = self.current_tick();
        self.0.borrow_mut().wait_until = Some(now + n);
    }

    pub fn current_tick(&self) -> u32 {
        simulation::Tick::fetch().value()
    }
}
