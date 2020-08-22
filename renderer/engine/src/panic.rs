use arrayvec::ArrayVec;
use backtrace::Backtrace;
use common::*;

use std::panic::PanicInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

lazy_static! {
    static ref HAS_PANICKED: AtomicBool = AtomicBool::default();
    static ref PANICS: Mutex<ArrayVec<[Panic; 16]>> = Mutex::new(ArrayVec::new());
}

// thread_local! {
//     static IS_MAIN_THREAD: AtomicBool = AtomicBool::default();
// }

#[derive(Debug)]
pub struct Panic {
    pub message: Option<String>,
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

pub fn panics() -> impl Iterator<Item = Panic> {
    let mut panics = PANICS.lock().unwrap();
    let stolen = std::mem::take(&mut *panics);
    stolen.into_iter()
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

    let message = panic.payload().downcast_ref::<&str>();

    error!("handling panic"; "thread" => &thread, "message" => message);

    let backtrace = Backtrace::new_unresolved();

    let message = message.map(|s| (*s).to_owned());

    HAS_PANICKED.store(true, Ordering::Relaxed);

    // TODO use mutex that cant be poisoned
    let mut panics = PANICS.lock().unwrap(); // nested panic!?
    if let Err(e) = panics.try_push(Panic {
        message,
        thread,
        backtrace,
    }) {
        error!("too many concurrent panics"; "count" => panics.len(), "error" => %e);
    }
}
