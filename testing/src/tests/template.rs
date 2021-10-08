use crate::tests::TestHelper;
use crate::{HookContext, HookResult, InitHookResult, TestDeclaration};

pub struct MyTest {

}

impl MyTest {
    pub fn on_tick(&mut self, test: TestHelper, _ctx: &HookContext) -> HookResult {
        todo!()
    }

    pub fn on_init(test: TestHelper, ctx: &HookContext) -> InitHookResult<Self> {
        InitHookResult::Success(Box::new(Self {

        }))
    }
}

declare_test!(MyTest);
