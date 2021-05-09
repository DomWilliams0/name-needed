use common::*;

#[derive(Error, Debug)]
pub enum ScriptingError {
    #[cfg(feature = "scripting")]
    #[error("Lua error: {0}")]
    Lua(#[from] rlua::Error),
}

pub trait Scripting: Sized {
    fn new() -> Result<Self, ScriptingError>;

    fn run(&mut self, script: &str) -> Result<(), ScriptingError>;
}

pub struct ScriptingContext<S: Scripting> {
    inner: S,
}

impl<S: Scripting> ScriptingContext<S> {
    pub fn new() -> Result<Self, ScriptingError> {
        let inner = S::new()?;
        Ok(Self { inner })
    }

    fn run(&mut self, script: &str) -> Result<(), ScriptingError> {
        self.inner.run(script)
    }
}
