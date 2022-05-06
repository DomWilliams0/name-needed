use common::*;
use std::fmt::Write;
use std::hash::BuildHasherDefault;
use ustr::{IdentityHasher, Ustr};

/// String interning for definitions and component names. Inserted as a resource into the world
/// and clears the global ustr cache on drop.
#[derive(Default)]
pub struct StringCache;

/// Interned string that lives as long as the owning simulation instance
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CachedStr(Ustr);

pub type CachedStringHasher = BuildHasherDefault<IdentityHasher>;

impl StringCache {
    pub fn get(&self, s: &str) -> CachedStr {
        CachedStr(ustr::ustr(s))
    }

    /// Doesn't require a StringCache resource
    #[cfg(any(test, feature = "testing"))]
    pub fn get_direct(s: &str) -> CachedStr {
        CachedStr(ustr::ustr(s))
    }
}

#[cfg(any(test, feature = "testing"))]
impl From<&str> for CachedStr {
    fn from(s: &str) -> Self {
        StringCache::get_direct(s)
    }
}

// dont bother clearing in tests
#[cfg(not(test))]
impl Drop for StringCache {
    fn drop(&mut self) {
        let bytes = ustr::total_allocated();
        let n = ustr::num_entries();
        debug!("freeing {n} strings ({bytes} bytes) in string cache");
        trace!("string cache: {:?}", self);
        // saftey: this is only dropped when the world is destroyed, and the game is over
        unsafe {
            ustr::_clear_cache();
        }
    }
}

impl AsRef<str> for CachedStr {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl CachedStr {
    pub fn as_ref_static(&self) -> &'static str {
        self.0.as_str()
    }
}

impl Display for CachedStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for CachedStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Debug for StringCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StringCache(n={}, bytes={}, strings=",
            ustr::num_entries(),
            ustr::total_allocated()
        )?;
        // TODO report panic when cache is empty
        if ustr::num_entries() > 0 {
            f.debug_list().entries(ustr::string_cache_iter()).finish()?;
        }
        f.write_char(')')
    }
}
