pub use sink::{deinit, init};

pub use crate::event::Verbosity;
pub use crate::event::{AiEvent, EntityId, Event, Span};

mod event;
mod r#macro;
pub mod sink;
