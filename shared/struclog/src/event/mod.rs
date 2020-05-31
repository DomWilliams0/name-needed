// mod entity;
mod ai;
mod span;
mod verbosity;

pub type EntityId = u64;
pub use ai::AiEvent;
pub use span::Span;
pub use verbosity::Verbosity;

use serde::Serialize;

#[derive(Serialize)]
#[non_exhaustive]
pub enum Event {
    CreateEntity(EntityId),
    Ai(AiEvent),
}

impl From<ai::AiEvent> for Event {
    fn from(ai: AiEvent) -> Self {
        Event::Ai(ai)
    }
}
