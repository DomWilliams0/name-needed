mod init;
mod tests;

#[cfg(feature = "elasticsearch")]
mod elastic;

pub use init::LoggerBuilder;
pub use tests::for_tests;

pub mod prelude {
    pub use slog_scope::crit as my_crit;
    pub use slog_scope::debug as my_debug;
    pub use slog_scope::error as my_error;
    pub use slog_scope::info as my_info;
    pub use slog_scope::trace as my_trace;
    pub use slog_scope::warn as my_warn;

    pub use slog::{
        self, b, o, Drain as SlogDrain, FnValue, Key, Level as MyLevel, Record,
        Result as SlogResult, Serializer,
    };

    pub use slog_scope::{self, log_scope, logger};
}

#[macro_export]
macro_rules! slog_value_debug {
    ($ty:ident) => {
        impl $crate::prelude::slog::Value for $ty {
            fn serialize(
                &self,
                _: &$crate::prelude::slog::Record,
                key: $crate::prelude::slog::Key,
                serializer: &mut dyn $crate::prelude::slog::Serializer,
            ) -> $crate::prelude::slog::Result<()> {
                serializer.emit_arguments(key, &format_args!("{:?}", self))
            }
        }
    };
}

#[macro_export]
macro_rules! slog_kv_debug {
    ($ty:ident, $key:expr) => {
        impl $crate::prelude::slog::KV for $ty {
            fn serialize(
                &self,
                _: &$crate::prelude::slog::Record,
                serializer: &mut dyn $crate::prelude::slog::Serializer,
            ) -> $crate::prelude::slog::Result<()> {
                serializer.emit_arguments($key, &format_args!("{:?}", self))
            }
        }
    };
}
