mod context;

pub use context::{Scripting, ScriptingError};

#[cfg(feature = "scripting")]
mod lua;

#[cfg(feature = "scripting")]
pub type ScriptingContext = context::ScriptingContext<lua::LuaScripting>;

#[cfg(not(feature = "scripting"))]
mod nop;

#[cfg(not(feature = "scripting"))]
pub type ScriptingContext = context::ScriptingContext<nop::NopScripting>;
