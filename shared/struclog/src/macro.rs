#[macro_export]
macro_rules! event_trace {
    ($event:expr) => {
        $crate::event!($event, $crate::Verbosity::Trace)
    };
}

#[macro_export]
macro_rules! event_verbose {
    ($event:expr) => {
        $crate::event!($event, $crate::Verbosity::Verbose)
    };
}

#[macro_export]
macro_rules! event_info {
    ($event:expr) => {
        $crate::event!($event, $crate::Verbosity::Info)
    };
}

#[macro_export]
macro_rules! event {
    ($event:expr,$verbosity:expr) => {
        if $crate::Verbosity::should_log_static($verbosity) {
            $crate::sink::post($event);
        }
    };
}

#[macro_export]
macro_rules! events_verbose {
    ($events:expr) => {
        $crate::events!($events, $crate::Verbosity::Verbose)
    };
}

#[macro_export]
macro_rules! events {
    ($events:expr,$verbosity:expr) => {
        if $crate::Verbosity::should_log_static($verbosity) {
            $crate::sink::post_batch($events);
        }
    };
}
