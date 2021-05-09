use rlua::{Lua, StdLib};

use crate::scripting::context::{Scripting, ScriptingError};

pub struct LuaScripting {
    runtime: rlua::Lua,
}

impl Scripting for LuaScripting {
    fn new() -> Result<Self, ScriptingError> {
        let std = StdLib::empty();
        let runtime = Lua::new_with(std);
        runtime.set_memory_limit(Some(10 * 1024 * 1024));
        Ok(Self { runtime })
    }

    fn run(&mut self, script: &str) -> Result<(), ScriptingError> {
        self.runtime
            .context(|ctx| ctx.load(script).exec())
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua() {
        let mut lua = LuaScripting::new().expect("failed");
        lua.run("myglobal = 5;").expect("failed");

        let value: i32 = lua
            .runtime
            .context(|ctx| {
                let g = ctx.globals();
                g.get("myglobal")
            })
            .expect("failed");

        assert_eq!(value, 5);
    }
}
