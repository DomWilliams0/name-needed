use crate::{sink, EntityId};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub enum Span {
    Setup,
    Tick,
    /// Interpolation
    Render(f64),
    /// Entity id for following entity events
    Entity(EntityId),
}

impl Span {
    pub fn begin(self) -> SpanGuard {
        sink::enter_span(self);
        SpanGuard
    }
}

pub struct SpanGuard;

impl Drop for SpanGuard {
    fn drop(&mut self) {
        sink::pop_span();
    }
}
