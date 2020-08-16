use std::error::Error;
use std::fmt::{Display, Formatter};

use slog::{Drain, Level};
use slog_scope::GlobalLoggerGuard;
use slog_term::ThreadSafeTimestampFn;

pub struct LoggerBuilder {
    level: Level,
}

pub struct Logger(Level, GlobalLoggerGuard);

#[derive(Debug)]
pub enum LogError {
    BadLevel(String),
}

impl LoggerBuilder {
    pub fn with_env() -> Result<Self, LogError> {
        let mut builder = Self::default();

        if let Ok(env) = std::env::var("NN_LOG") {
            let level = env.parse().map_err(|_| LogError::BadLevel(env.clone()))?;
            builder = builder.level(level)
        }

        Ok(builder)
    }

    pub fn level(mut self, s: Level) -> Self {
        self.level = s;
        self
    }

    // TODO configure to write to file as text
    // TODO configure to publish to elasticsearch

    pub fn init(self, timestamp_fn: impl ThreadSafeTimestampFn) -> Result<Logger, LogError> {
        let decorator = slog_term::TermDecorator::new()
            .stderr()
            .force_color()
            .build();
        let drain = slog_term::CompactFormat::new(decorator)
            .use_custom_timestamp(timestamp_fn)
            .build()
            .fuse();
        let drain = drain.filter_level(self.level).fuse();
        let drain = slog_async::Async::new(drain)
            .thread_name("logging".to_owned())
            .chan_size(1024)
            .build_no_guard()
            .fuse();
        let logger = slog::Logger::root(drain, slog::o!());

        let global = slog_scope::set_global_logger(logger);
        Ok(Logger(self.level, global))
    }
}

impl Default for LoggerBuilder {
    fn default() -> Self {
        Self { level: Level::Info }
    }
}

impl Logger {
    pub fn level(&self) -> Level {
        self.0
    }
}

impl Display for LogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LogError::BadLevel(s) => write!(f, "Invalid level {:?}", s),
        }
    }
}

impl Error for LogError {}
