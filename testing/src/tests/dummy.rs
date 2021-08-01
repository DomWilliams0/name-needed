use crate::{HookContext, HookResult, TestDeclaration};

#[derive(Default)]
pub struct Dummy(usize);

impl Dummy {
    pub fn on_tick(&mut self, _: &HookContext) -> HookResult {
        self.0 += 1;

        if self.0 >= 40 {
            HookResult::TestSuccess
        } else {
            HookResult::KeepGoing
        }
    }

    pub fn on_init(&mut self, _: &HookContext) -> HookResult {
        HookResult::KeepGoing
    }
}

declare_test!(Dummy);
