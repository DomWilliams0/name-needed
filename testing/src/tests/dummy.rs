use crate::tests::TestHelper;
use crate::{HookContext, HookResult, InitHookResult, TestDeclaration};

const MAGIC: u64 = 0xab846de193857a02;

pub struct Dummy(u64);

impl Dummy {
    pub fn on_tick(&mut self, test: TestHelper, _ctx: &HookContext) -> HookResult {
        assert_eq!(self.0, MAGIC, "self is corrupt");

        let tick = test.current_tick();
        assert!(tick >= 16);
        HookResult::TestSuccess
    }

    pub fn on_init(test: TestHelper, _ctx: &HookContext) -> InitHookResult<Self> {
        test.wait_n_ticks(16);
        InitHookResult::Success(Box::new(Self(MAGIC)))
    }
}

impl Drop for Dummy {
    fn drop(&mut self) {
        assert_eq!(self.0, MAGIC, "self is corrupt in destructor");
        common::debug!("running Dummy destructor!");
    }
}

declare_test!(Dummy);
