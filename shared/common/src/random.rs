//! Deterministic random generator seeded from config and shared between all threads, only use for
//! things that really need to be deterministic
//!
use crate::*;
use std::ops::DerefMut;
use std::sync::{Mutex, MutexGuard};

lazy_static! {
    static ref RANDY: Mutex<StdRng> = Mutex::new(StdRng::from_entropy());
}

pub fn reseed(seed: u64) {
    let mut randy = RANDY.lock().unwrap();
    *randy.deref_mut() = StdRng::seed_from_u64(seed);
}

/// May block!!
pub fn get<'a>() -> MutexGuard<'a, StdRng> {
    RANDY.lock().unwrap()
}
