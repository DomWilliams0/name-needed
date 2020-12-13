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

#[derive(Debug)]
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
    let mut panics = PANICS.lock();
    std::mem::take(&mut *panics)
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

pub fn run_and_handle_panics<R>(do_me: impl FnOnce() -> R + UnwindSafe) -> Result<R, ()> {
    let result = std::panic::catch_unwind(|| do_me());

    let all_panics = panics();

    debug_assert_eq!(all_panics.is_empty(), result.is_ok());

    result.map_err(|_| {
        crit!("{count} threads panicked", count = all_panics.len());

        for Panic {
            message,
            thread,
            mut backtrace,
        } in all_panics
        {
            backtrace.resolve();

            crit!("panic";
            "backtrace" => ?backtrace,
            "message" => message,
            "thread" => thread,
            );
        }
    })
}
