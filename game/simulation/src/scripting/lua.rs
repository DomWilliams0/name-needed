use rlua::{Lua, StdLib, Variadic};

use crate::scripting::context::{Scripting, ScriptingError, ScriptingResult};

pub struct LuaScripting {
    runtime: rlua::Lua,
}

impl Scripting for LuaScripting {
    fn new() -> Result<Self, ScriptingError> {
        let std = {
            let mut std = StdLib::ALL_NO_DEBUG;
            std.remove(StdLib::COROUTINE);
            std
        };
        let runtime = Lua::new_with(std);
        runtime.set_memory_limit(Some(10 * 1024 * 1024));
        // TODO configure lua GC

        runtime.context(|ctx| {
            let log_print = ctx.create_function(|_, msg: String| {
                common::info!("lua: {}", msg);
                Ok(())
            })?;

            ctx.globals().set("print", log_print)?;

            ScriptingResult::Ok(())
        })?;

        Ok(Self { runtime })
    }

    fn run(&mut self, script: &[u8]) -> Result<(), ScriptingError> {
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
        lua.run("myglobal = 5;".as_ref()).expect("failed");

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
