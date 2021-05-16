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
    Io(std::io::Error),
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

    pub fn init(self, timestamp_fn: impl ThreadSafeTimestampFn + Copy) -> Result<Logger, LogError> {
        let terminal_drain = {
            let decorator = slog_term::TermDecorator::new()
                .stderr()
                .force_color()
                .build();
            slog_term::CompactFormat::new(decorator)
                .use_custom_timestamp(timestamp_fn)
                .build()
                .filter_level(self.level.min(Level::Debug)) // dont spam terminal with trace
                .fuse()
        };

        let drain = {
            #[cfg(feature = "to-file")]
            {
                use std::fs::OpenOptions;

                let file_drain = {
                    let log_file = std::env::temp_dir().join("nn-logs");

                    if log_file.is_file() {
                        // try to backup
                        let mut bak = log_file.clone();
                        bak.set_extension("bak");
                        let _ = std::fs::rename(&log_file, &bak);
                    }

                    let file = OpenOptions::new()
                        .create(true)
                        .truncate(true)
                        .write(true)
                        .open(log_file)
                        .map_err(LogError::Io)?;
                    let decorator = slog_term::PlainDecorator::new(file);
                    slog_term::FullFormat::new(decorator)
                        .use_custom_timestamp(timestamp_fn)
                        .build()
                        .fuse()
                };

                slog::Duplicate::new(terminal_drain, file_drain)
            }

            #[cfg(not(feature = "to-file"))]
            terminal_drain
        };
        let chan_size = match self.level {
            Level::Debug | Level::Trace => 0x20000,
            _ => 0x4000,
        };

        let drain = drain.filter_level(self.level).fuse();
        let drain = slog_async::Async::new(drain)
            .thread_name("logging".to_owned())
            .chan_size(chan_size)
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

    pub fn logger(&self) -> slog::Logger {
        slog_scope::logger()
    }
}

impl Display for LogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LogError::BadLevel(s) => write!(f, "Invalid level {:?}", s),
            LogError::Io(e) => write!(f, "Io error opening log file: {}", e),
        }
    }
}

impl Error for LogError {}
