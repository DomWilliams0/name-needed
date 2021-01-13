use crate::*;
use backtrace::Backtrace;

use parking_lot::Mutex;
use std::borrow::Cow;
use std::panic::{PanicInfo, UnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};

lazy_static! {
    static ref HAS_PANICKED: AtomicBool = AtomicBool::default();
    static ref PANICS: Mutex<Vec<Panic>> = Mutex::new(Vec::new());
}

#[derive(Debug, Clone)]
pub struct Panic {
    pub message: String,
    pub thread: String,
    pub backtrace: Backtrace,
}

pub fn init_panic_detection() {
    std::panic::set_hook(Box::new(|panic| {
        register_panic(panic);
    }));

    info!("initialized panic handler");
}

pub fn panics() -> Vec<Panic> {
    let panics = PANICS.lock();
    panics.clone() // efficiency be damned we're dying
}

pub fn has_panicked() -> bool {
    HAS_PANICKED.load(Ordering::Relaxed)
}

fn register_panic(panic: &PanicInfo) {
    let thread = {
        let t = std::thread::current();
        let name = t.name().unwrap_or("<unnamed>");
        format!("{:?} ({})", t.id(), name)
    };

    // TODO use panic.message() when it stabilises
    let message = panic
        .payload()
        .downcast_ref::<&str>()
        .map(|s| Cow::Borrowed(*s))
        .unwrap_or_else(|| Cow::from(format!("{}", panic)));

    error!("handling panic"; "thread" => &thread, "message" => %message);

    let backtrace = Backtrace::new_unresolved();

    HAS_PANICKED.store(true, Ordering::Relaxed);

    let mut panics = PANICS.lock();
    panics.push(Panic {
        message: message.into_owned(),
        thread,
        backtrace,
    });
}

/// None on error
pub fn run_and_handle_panics<R: Debug>(do_me: impl FnOnce() -> R + UnwindSafe) -> Option<R> {
    let result = std::panic::catch_unwind(|| do_me());

    let all_panics = panics();

    match (result, all_panics.is_empty()) {
        (Ok(res), true) => {
            // no panics
            return Some(res);
        }
        (Ok(res), false) => {
            warn!("panic occurred in another thread, swallowing unpanicked result"; "result" => ?res);
        }
        (Err(_), false) => {}
        (Err(_), true) => unreachable!(),
    };

    debug_assert!(!all_panics.is_empty());
    crit!("{count} threads panicked", count = all_panics.len());

    const BACKTRACE_RESOLUTION_LIMIT: usize = 8;
    for (
        i,
        Panic {
            message,
            thread,
            mut backtrace,
        },
    ) in all_panics.into_iter().enumerate()
    {
        if i == BACKTRACE_RESOLUTION_LIMIT {
            warn!(
                "handling more than {limit} panics, no longer resolving backtraces",
                limit = BACKTRACE_RESOLUTION_LIMIT
            );
        }
        if i < BACKTRACE_RESOLUTION_LIMIT {
            backtrace.resolve();
        }

        crit!("panic";
        "backtrace" => ?backtrace,
        "message" => message,
        "thread" => thread,
        );
    }

    None
}
