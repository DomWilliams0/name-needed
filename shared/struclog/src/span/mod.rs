use crate::sink;
#[cfg(feature = "ipc")]
use serde::Serialize;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "ipc", derive(Serialize))]
pub enum Span {
    Setup,
    Tick,
    Render,
    Physics,
}

pub struct SpanGuard;

impl Drop for SpanGuard {
    fn drop(&mut self) {
        sink::pop_span();
    }
}
