use crate::tests::TestHelper;
use crate::{HookContext, HookResult, TestDeclaration};

#[derive(Default)]
pub struct Dummy;

impl Dummy {
    pub fn on_tick(&mut self, test: TestHelper, _ctx: &HookContext) -> HookResult {
        let tick = test.current_tick();
        assert!(tick >= 123);
        HookResult::TestSuccess
    }

    pub fn on_init(&mut self, test: TestHelper, _ctx: &HookContext) -> HookResult {
        test.wait_n_ticks(123);
        HookResult::KeepGoing
    }
}

declare_test!(Dummy);
