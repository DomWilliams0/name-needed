//! Deterministic random generator seeded from config and shared between all threads, only use for
//! things that really need to be deterministic
//!
use crate::*;
use parking_lot::{Mutex, MutexGuard};
use std::ops::DerefMut;

lazy_static! {
    static ref RANDY: Mutex<SmallRng> = Mutex::new(SmallRng::from_entropy());
}

pub fn reseed(seed: u64) {
    let mut randy = RANDY.lock();
    *randy.deref_mut() = SmallRng::seed_from_u64(seed);
}

/// May block!! In debug builds panics on deadlock
pub fn get<'a>() -> MutexGuard<'a, SmallRng> {
    if cfg!(debug_assertions) {
        RANDY
            .try_lock()
            .unwrap_or_else(|| panic!("can't take the random mutex"))
    } else {
        RANDY.lock()
    }
}

pub trait SmallRngExt {
    /// Uses thread rng as seed, to avoid going through the OS getrandom, which is way slower and
    /// more secure than we need
    fn new_quick() -> SmallRng;
}

impl SmallRngExt for SmallRng {
    fn new_quick() -> SmallRng {
        SmallRng::from_rng(thread_rng()).expect("failed to seed quick rng")
    }
}
