use common::*;
use std::path::Path;

#[derive(Error, Debug)]
pub enum ScriptingError {
    #[cfg(feature = "scripting")]
    #[error("Lua error: {0}")]
    Lua(#[from] rlua::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type ScriptingResult<T> = Result<T, ScriptingError>;

pub trait Scripting: Sized {
    fn new() -> ScriptingResult<Self>;

    fn run(&mut self, script: &[u8]) -> ScriptingResult<()>;
}

pub struct ScriptingContext<S: Scripting> {
    inner: S,
}

impl<S: Scripting> ScriptingContext<S> {
    pub fn new() -> ScriptingResult<Self> {
        let inner = S::new()?;
        Ok(Self { inner })
    }

    pub fn eval_path(&mut self, path: &Path) -> ScriptingResult<()> {
        let bytes = std::fs::read(path)?;
        self.eval(&bytes)
    }

    fn eval(&mut self, script: &[u8]) -> ScriptingResult<()> {
        self.inner.run(script)
    }
}
