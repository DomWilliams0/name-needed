mod entity;

pub use entity::{EntityEvent, EntityId};

#[cfg(feature = "ipc")]
use serde::Serialize;

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ipc", derive(Serialize))]
pub enum Event {
    Entity(EntityEvent),
}

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub enum Verbosity {
    Error,
    Info,
    Verbose,
    Trace,
}

#[cfg(feature = "error")]
const VERBOSITY: Verbosity = Verbosity::Error;
#[cfg(feature = "info")]
const VERBOSITY: Verbosity = Verbosity::Info;
#[cfg(feature = "verbose")]
const VERBOSITY: Verbosity = Verbosity::Verbose;
#[cfg(feature = "trace")]
const VERBOSITY: Verbosity = Verbosity::Trace;
#[cfg(
    all(test, not(any(feature = "error", feature = "info", feature = "verbose", feature = "trace")))
)]
const VERBOSITY: Verbosity = Verbosity::Info;

impl Verbosity {
    #[inline(always)]
    pub fn should_log(self, sink: Verbosity) -> bool {
        self <= sink
    }

    #[inline(always)]
    pub fn should_log_static(self) -> bool {
        self.should_log(VERBOSITY)
    }

    pub const fn get() -> Self {
        VERBOSITY
    }
}

#[cfg(test)]
mod tests {
    use crate::event::Verbosity;
    use crate::event::Verbosity::*;

    #[test]
    fn compare() {
        let mut sink;

        sink = Trace;
        assert!(Trace.should_log(sink));
        assert!(Verbose.should_log(sink));
        assert!(Info.should_log(sink));
        assert!(Error.should_log(sink));

        sink = Verbose;
        assert!(!Trace.should_log(sink));
        assert!(Verbose.should_log(sink));
        assert!(Info.should_log(sink));
        assert!(Error.should_log(sink));

        sink = Info;
        assert!(!Trace.should_log(sink));
        assert!(!Verbose.should_log(sink));
        assert!(Info.should_log(sink));
        assert!(Error.should_log(sink));

        sink = Error;
        assert!(!Trace.should_log(sink));
        assert!(!Verbose.should_log(sink));
        assert!(!Info.should_log(sink));
        assert!(Error.should_log(sink));
    }
}
