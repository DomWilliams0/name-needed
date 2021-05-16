use crate::scripting::context::{Scripting, ScriptingError, ScriptingOutput, ScriptingResult};
use crate::{EcsWorld, WorldRef};

pub struct NopScripting;

impl Scripting for NopScripting {
    fn new() -> Result<Self, ScriptingError> {
        Ok(NopScripting)
    }

    fn run(
        &mut self,
        _script: &[u8],
        _ecs: &EcsWorld,
        _world: &WorldRef,
    ) -> ScriptingResult<ScriptingOutput> {
        Ok(ScriptingOutput::default())
    }
}
