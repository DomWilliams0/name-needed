//! Deterministic random generator seeded from config and shared between all threads, only use for
//! things that really need to be deterministic
//!
use crate::*;
use parking_lot::{Mutex, MutexGuard};
use std::ops::DerefMut;

lazy_static! {
    static ref RANDY: Mutex<StdRng> = Mutex::new(StdRng::from_entropy());
}

pub fn reseed(seed: u64) {
    let mut randy = RANDY.lock();
    *randy.deref_mut() = StdRng::seed_from_u64(seed);
}

/// May block!! In debug builds panics on deadlock
pub fn get<'a>() -> MutexGuard<'a, StdRng> {
    if cfg!(debug_assertions) {
        RANDY
            .try_lock()
            .unwrap_or_else(|| panic!("can't take the random mutex"))
    } else {
        RANDY.lock()
    }
}
