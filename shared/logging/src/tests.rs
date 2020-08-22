use std::sync::Mutex;

use once_cell::sync::OnceCell;
use slog::Drain;
use slog_scope::GlobalLoggerGuard;

static LOGGER: OnceCell<GlobalLoggerGuard> = OnceCell::new();

/// Only works for running a single test :(
#[allow(dead_code)]
pub fn for_tests() {
    LOGGER.get_or_init(|| {
        // let drain = StdLog.filter_level(Level::Trace).fuse();
        let drain = slog_term::TermDecorator::new()
            .stdout()
            .force_color()
            .build();
        let drain = slog_term::CompactFormat::new(drain).build();
        let drain = Mutex::new(drain).fuse();
        let logger = slog::Logger::root(drain, slog::o!());
        slog_scope::set_global_logger(logger)
    });
}
