use std::sync::Mutex;

use once_cell::sync::OnceCell;
use slog::{Drain, Level};
use slog_scope::GlobalLoggerGuard;
use std::fs::OpenOptions;

static LOGGER: OnceCell<GlobalLoggerGuard> = OnceCell::new();

/// Only works for running a single test :(
#[allow(dead_code)]
pub fn for_tests() {
    LOGGER.get_or_init(|| {
        // let drain = StdLog.filter_level(Level::Trace).fuse();
        let drain = slog_term::TermDecorator::new()
            .stderr()
            .force_color()
            .build();
        let terminal_drain = slog_term::CompactFormat::new(drain).build();

        let file_drain = {
            let log_file = std::env::temp_dir().join("nn-test-logs");
            let file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(log_file)
                .expect("failed to create log file");
            let decorator = slog_term::PlainDecorator::new(file);
            slog_term::FullFormat::new(decorator).build().fuse()
        };

        let drain = slog::Duplicate::new(terminal_drain, file_drain);

        let drain = Mutex::new(drain.filter_level(Level::Trace)).fuse();
        let logger = slog::Logger::root(drain, slog::o!());
        slog_scope::set_global_logger(logger)
    });
}
