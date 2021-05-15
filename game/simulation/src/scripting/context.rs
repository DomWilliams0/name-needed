use crate::ecs::EntityWrapper;
use crate::{EcsWorld, WorldRef};
use common::*;
use std::fmt::Write;
use std::path::Path;

#[derive(Error, Debug)]
pub enum ScriptingError {
    #[cfg(feature = "scripting")]
    #[error("Lua error: {0}")]
    Lua(#[from] rlua::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid entity ID: {0}")]
    InvalidEntityId(String),

    #[error("Entity is not alive: {0}")]
    DeadEntity(EntityWrapper),
}

#[derive(Default)]
pub struct ScriptingOutput(String);

pub type ScriptingResult<T> = Result<T, ScriptingError>;

pub trait Scripting: Sized {
    fn new() -> ScriptingResult<Self>;

    fn run(
        &mut self,
        script: &[u8],
        ecs: &EcsWorld,
        world: &WorldRef,
    ) -> ScriptingResult<ScriptingOutput>;
}

pub struct ScriptingContext<S: Scripting> {
    inner: S,
}

impl<S: Scripting> ScriptingContext<S> {
    pub fn new() -> ScriptingResult<Self> {
        let inner = S::new()?;
        Ok(Self { inner })
    }

    pub fn eval_path(
        &mut self,
        path: &Path,
        ecs: &EcsWorld,
        world: &WorldRef,
    ) -> ScriptingResult<ScriptingOutput> {
        let bytes = std::fs::read(path)?;
        self.eval_bytes(&bytes, ecs, world)
    }

    fn eval_bytes(
        &mut self,
        bytes: &[u8],
        ecs: &EcsWorld,
        world: &WorldRef,
    ) -> ScriptingResult<ScriptingOutput> {
        self.inner.run(bytes, ecs, world)
    }
}

/// Expects E{gen}:{idx} format e.g. E1:2, E20:15
pub fn parse_entity_id(e: &str) -> Option<EntityWrapper> {
    match e.bytes().next() {
        Some(b'E') => {}
        _ => return None,
    };

    let colon = e.chars().skip(1).position(|b| b == ':')? + 1;

    let gen_str = &e[1..colon];
    let index_str = e.get((colon + 1)..)?;

    let gen = gen_str.parse().ok()?;
    let index = index_str.parse().ok()?;

    Some(EntityWrapper(index, gen))
}

impl ScriptingOutput {
    pub fn add_line(&mut self, line: std::fmt::Arguments) {
        self.0.write_fmt(line).expect("string writing failed");
        self.0.push('\n');
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroI32;

    #[test]
    fn parse_entity() {
        assert!(parse_entity_id("").is_none());
        assert!(parse_entity_id("E").is_none());
        assert!(parse_entity_id("asdf").is_none());
        assert!(parse_entity_id("1234").is_none());
        assert!(parse_entity_id("E123").is_none());
        assert!(parse_entity_id("E123:").is_none());
        assert!(parse_entity_id("E123;123").is_none());
        assert!(parse_entity_id("E123:567abc").is_none());
        assert!(parse_entity_id("E0:5").is_none()); // non zero generation

        assert_eq!(
            parse_entity_id("E123:567"),
            Some(EntityWrapper(567, NonZeroI32::new(123).unwrap()))
        );

        assert_eq!(
            parse_entity_id("E1:4"),
            Some(EntityWrapper(4, NonZeroI32::new(1).unwrap()))
        );
    }
}
