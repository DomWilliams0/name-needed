use crate::scripting::context::{Scripting, ScriptingError, ScriptingResult};
use crate::{EcsWorld, WorldRef};

pub struct NopScripting;

impl Scripting for NopScripting {
    fn new() -> Result<Self, ScriptingError> {
        Ok(NopScripting)
    }

    fn run(&mut self, script: &[u8], ecs: &EcsWorld, world: &WorldRef) -> ScriptingResult<()> {
        Ok(())
    }
}
