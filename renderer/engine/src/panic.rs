use backtrace::Backtrace;
use common::*;

use parking_lot::Mutex;
use std::borrow::Cow;
use std::panic::PanicInfo;
use std::sync::atomic::{AtomicBool, Ordering};

lazy_static! {
    static ref HAS_PANICKED: AtomicBool = AtomicBool::default();
    static ref PANICS: Mutex<Vec<Panic>> = Mutex::new(Vec::new());
}

// thread_local! {
//     static IS_MAIN_THREAD: AtomicBool = AtomicBool::default();
// }

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

    // IS_MAIN_THREAD.with(|b| {
    //     b.store(true, Ordering::Relaxed);
    // });

    info!("initialized panic handler");
}

pub fn panics() -> Vec<Panic> {
    let mut panics = PANICS.lock();
    std::mem::take(&mut *panics)
}

pub fn has_panicked() -> bool {
    HAS_PANICKED.load(Ordering::Relaxed)
}

// fn is_main_thread() -> bool {
//     IS_MAIN_THREAD.with(|b| b.load(Ordering::Relaxed))
// }

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
