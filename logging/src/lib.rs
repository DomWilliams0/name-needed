#[cfg(feature = "binary")]
mod init;

#[cfg(feature = "binary")]
pub use init::LoggerBuilder;

// can't be cfg(test) because this is used as a dependency in tested crates, and so isn't compiled
// with cfg(test)
mod tests;
pub use tests::for_tests;

pub mod prelude {
    pub use slog_scope::crit;
    pub use slog_scope::debug;
    pub use slog_scope::error;
    pub use slog_scope::info;
    pub use slog_scope::trace;
    pub use slog_scope::warn;

    pub use slog::{
        self, b, o, Drain as SlogDrain, FnValue, Key, Level as MyLevel, Record,
        Result as SlogResult, Serializer,
    };

    pub use slog_scope::{self, log_scope, logger};
}

#[macro_export]
macro_rules! slog_value_debug {
    ($ty:ty) => {
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
    ($ty:ty, $key:expr) => {
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

#[macro_export]
macro_rules! slog_value_display {
    ($ty:ty) => {
        impl $crate::prelude::slog::Value for $ty {
            fn serialize(
                &self,
                _: &$crate::prelude::slog::Record,
                key: $crate::prelude::slog::Key,
                serializer: &mut dyn $crate::prelude::slog::Serializer,
            ) -> $crate::prelude::slog::Result<()> {
                serializer.emit_arguments(key, &format_args!("{}", self))
            }
        }
    };
}

#[macro_export]
macro_rules! slog_kv_display {
    ($ty:ty, $key:expr) => {
        impl $crate::prelude::slog::KV for $ty {
            fn serialize(
                &self,
                _: &$crate::prelude::slog::Record,
                serializer: &mut dyn $crate::prelude::slog::Serializer,
            ) -> $crate::prelude::slog::Result<()> {
                serializer.emit_arguments($key, &format_args!("{}", self))
            }
        }
    };
}
