// tests must be registered in lib::register_tests!

use crate::{HookResult, InitHookResult, TestInstance};
use std::any::Any;
use std::cell::RefCell;
use std::mem::{ManuallyDrop, MaybeUninit};

// ---------- test modules
pub mod dummy;
pub mod equip_with_pickup;
pub mod haul;
// ---------- end test modules

pub struct TestWrapper {
    /// None before init is called. Is not actually a () but a test type, so must be manually dropped
    test: RefCell<Option<ManuallyDrop<Box<()>>>>,
    state: RefCell<TestState>,
}

#[derive(Default)]
struct TestState {
    /// Next tick to call on_tick
    wait_until: Option<u32>,
}

pub struct TestHelper<'a>(&'a RefCell<TestState>);

impl TestWrapper {
    pub fn new() -> Self {
        Self {
            test: RefCell::new(None),
            state: RefCell::new(TestState::default()),
        }
    }

    pub fn invoke_init(&self, do_it: impl FnOnce() -> InitHookResult<()>) -> HookResult {
        let mut this = self.test.borrow_mut();
        assert!(this.is_none(), "test init called multiple times");

        let result = do_it();

        match result {
            InitHookResult::Success(instance) => {
                *this = Some(ManuallyDrop::new(instance));
                HookResult::KeepGoing
            }
            InitHookResult::TestSuccess => HookResult::TestSuccess,
            InitHookResult::TestFailure(err) => HookResult::TestFailure(err),
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
        let this = this.as_mut().expect("init was not called");
        let this: &mut () = &mut **this;
        do_it(this)
    }

    pub fn helper(&self) -> TestHelper {
        TestHelper(&self.state)
    }

    /// Not really Box<()>, but Box<test type>
    pub fn take_inner(&mut self) -> Option<ManuallyDrop<Box<()>>> {
        self.test.take()
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
