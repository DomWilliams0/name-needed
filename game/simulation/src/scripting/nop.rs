use crate::scripting::context::{Scripting, ScriptingError};

pub struct NopScripting;

impl Scripting for NopScripting {
    fn new() -> Result<Self, ScriptingError> {
        Ok(NopScripting)
    }

    fn run(&mut self, _: &str) -> Result<(), ScriptingError> {
        Ok(())
    }
}
