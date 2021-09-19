use crate::helpers::EntityPosition;
use crate::tests::TestHelper;
use crate::{HookContext, HookResult, TestDeclaration};
use common::BoxedResult;
use simulation::{ComponentWorld, ConditionComponent};

#[derive(Default)]
pub struct EquipWithPickup;

impl EquipWithPickup {
    pub fn on_tick(&mut self, test: TestHelper, _ctx: &HookContext) -> HookResult {
        // TODO
        HookResult::TestSuccess
    }

    pub fn on_init(&mut self, test: TestHelper, ctx: &HookContext) -> HookResult {
        HookResult::try_ongoing(Self::setup(ctx))
    }

    fn setup(ctx: &HookContext) -> BoxedResult<()> {
        let human = ctx.new_human(EntityPosition::Origin)?;
        let item = ctx.new_entity("core_food_apple", EntityPosition::Far)?;

        // TODO actually implement the test
        // TODO reuse scenario helpers?

        Ok(())
    }
}

declare_test!(EquipWithPickup);

// TODO make available to all tests, or even scenarios too
mod helpers {}
