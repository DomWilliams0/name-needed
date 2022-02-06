use crate::scripting::context::{Scripting, ScriptingError, ScriptingOutput, ScriptingResult};
use crate::EcsWorld;

pub struct NopScripting;

impl Scripting for NopScripting {
    fn new() -> Result<Self, ScriptingError> {
        Ok(NopScripting)
    }

    fn run(&mut self, _script: &[u8], _ecs: &EcsWorld) -> ScriptingResult<ScriptingOutput> {
        Ok(ScriptingOutput::default())
    }
}
