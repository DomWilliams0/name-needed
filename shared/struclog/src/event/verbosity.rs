#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
#[repr(u8)]
pub enum Verbosity {
    Info,
    Verbose,
    Trace,
}

#[cfg(feature = "info")]
const VERBOSITY: Verbosity = Verbosity::Info;
#[cfg(feature = "verbose")]
const VERBOSITY: Verbosity = Verbosity::Verbose;
#[cfg(feature = "trace")]
const VERBOSITY: Verbosity = Verbosity::Trace;

/// Default for tests
#[cfg(not(any(feature = "info", feature = "verbose", feature = "trace")))]
const VERBOSITY: Verbosity = Verbosity::Info;

impl Verbosity {
    #[cfg(test)]
    fn should_log(self, sink: Verbosity) -> bool {
        self <= sink
    }

    #[inline]
    pub const fn should_log_static(self) -> bool {
        const LEVEL: u8 = VERBOSITY as u8;
        (self as u8) <= LEVEL
    }

    pub const fn get() -> Self {
        VERBOSITY
    }
}

#[cfg(test)]
mod tests {
    use crate::event::verbosity::Verbosity::*;

    #[test]
    fn compare() {
        let mut sink;

        sink = Trace;
        assert!(Trace.should_log(sink));
        assert!(Verbose.should_log(sink));
        assert!(Info.should_log(sink));

        sink = Verbose;
        assert!(!Trace.should_log(sink));
        assert!(Verbose.should_log(sink));
        assert!(Info.should_log(sink));

        sink = Info;
        assert!(!Trace.should_log(sink));
        assert!(!Verbose.should_log(sink));
        assert!(Info.should_log(sink));
    }
}
