use crate::world_ref::introspect::Timer;
use crate::{World, WorldContext};
use misc::derive_more::{Deref, DerefMut};
use misc::parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;
use tokio::runtime::Handle;

/// Reference counted reference to the world. Lock is not async so dont hold for long
pub struct WorldRef<C: WorldContext> {
    world: Arc<RwLock<World<C>>>,
    /// Handle of path finding runtime, accessible without needing to lock the world
    nav_runtime: Handle,
}

#[derive(Deref)]
pub struct InnerWorldRef<'a, C: WorldContext> {
    #[deref]
    guard: RwLockReadGuard<'a, World<C>>,
    #[allow(dead_code)]
    timer: Timer,
}

#[derive(Deref, DerefMut)]
pub struct InnerWorldRefMut<'a, C: WorldContext> {
    #[deref]
    #[deref_mut]
    guard: RwLockWriteGuard<'a, World<C>>,
    #[allow(dead_code)]
    timer: Timer,
}

impl<C: WorldContext> WorldRef<C> {
    pub fn new(world: World<C>) -> Self {
        let nav_runtime = world.nav_graph().pathfinding_runtime();
        Self {
            world: Arc::new(RwLock::new(world)),
            nav_runtime,
        }
    }

    pub fn borrow(&self) -> InnerWorldRef<'_, C> {
        let timer = Timer::wait("borrow");
        let guard = self.world.read();
        drop(timer);
        InnerWorldRef::with_guard(guard)
    }

    pub fn borrow_mut(&self) -> InnerWorldRefMut<'_, C> {
        let timer = Timer::wait("borrow_mut");
        let guard = self.world.write();
        drop(timer);
        InnerWorldRefMut::with_guard(guard)
    }

    pub fn nav_runtime(&self) -> Handle {
        self.nav_runtime.clone()
    }

    #[cfg(test)]
    pub fn into_inner(self) -> World<C> {
        let mutex = Arc::try_unwrap(self.world).unwrap_or_else(|arc| {
            panic!(
                "exclusive world reference needed but there are {}",
                Arc::strong_count(&arc)
            )
        });
        mutex.into_inner()
    }
}

impl<'a, C: WorldContext> InnerWorldRef<'a, C> {
    fn with_guard(guard: RwLockReadGuard<'a, World<C>>) -> Self {
        Self {
            guard,
            timer: Timer::hold("read-only borrow"),
        }
    }
}

impl<'a, C: WorldContext> InnerWorldRefMut<'a, C> {
    fn with_guard(guard: RwLockWriteGuard<'a, World<C>>) -> Self {
        Self {
            guard,
            timer: Timer::hold("mutable borrow"),
        }
    }
}

impl<'a, C: WorldContext> InnerWorldRefMut<'a, C> {
    pub fn downgrade(self) -> InnerWorldRef<'a, C> {
        let guard = RwLockWriteGuard::downgrade(self.guard);
        drop(self.timer);
        InnerWorldRef::with_guard(guard)
    }
}

impl<C: WorldContext> Default for WorldRef<C> {
    fn default() -> Self {
        WorldRef::new(World::default())
    }
}
impl<C: WorldContext> Clone for WorldRef<C> {
    fn clone(&self) -> Self {
        WorldRef {
            world: Arc::clone(&self.world),
            nav_runtime: self.nav_runtime.clone(),
        }
    }
}

#[cfg(not(feature = "log-lock-delay"))]
mod introspect {
    pub struct Timer;
    impl Timer {
        pub fn wait(_: &'static str) -> Self {
            Self
        }
        pub fn hold(_: &'static str) -> Self {
            Self
        }
    }
}

#[cfg(feature = "log-lock-delay")]
mod introspect {
    use misc::warn;
    use std::time::{Duration, Instant};

    #[inline(never)]
    fn callers(n: usize) -> Vec<String> {
        let mut callers = vec![];
        let mut seen_this_yet = false;
        let mut left = 14;
        backtrace::trace(|f| {
            backtrace::resolve_frame(f, |s| {
                match s.name().map(|s| format!("{}", s)) {
                    None => left = 0, // give up immediately
                    Some(name)
                        if !seen_this_yet && name.starts_with("world::world_ref::introspect") =>
                    {
                        seen_this_yet = true;
                    }
                    Some(name)
                        if seen_this_yet
                            && callers.len() < n
                            && !name.contains("world::world_ref")
                            && !name.contains("core::future") =>
                    {
                        // actual useful symbol, finally
                        callers.push(name.to_owned());
                    }
                    _ => {}
                };
                left -= 1;
            });
            left > 0 && callers.len() <= n
        });

        callers
    }

    #[derive(Copy, Clone)]
    enum TimeReason {
        Waiting,
        Held,
    }

    pub struct Timer {
        start: Instant,
        scope: &'static str,
        reason: TimeReason,
    }

    impl Timer {
        pub fn wait(scope: &'static str) -> Self {
            Self {
                start: Instant::now(),
                scope,
                reason: TimeReason::Waiting,
            }
        }
        pub fn hold(scope: &'static str) -> Self {
            Self {
                start: Instant::now(),
                scope,
                reason: TimeReason::Held,
            }
        }
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            let elapsed = self.start.elapsed();
            let thread = std::thread::current();
            let thread_name = thread.name();
            let limit = match (self.reason, thread_name) {
                (TimeReason::Waiting, Some("main")) => 5,
                (TimeReason::Held, _) => 25,
                _ => 100,
            };
            if elapsed > Duration::from_millis(limit) {
                let caller = callers(3).join(" >> ");

                let msg = match self.reason {
                    TimeReason::Waiting => "took to long to take lock",
                    TimeReason::Held => "held lock for too long",
                };
                misc::trace!(
                    "{msg} for {} on thread {} ({}ms)",
                    self.scope,
                    thread_name.unwrap_or("<unnamed>"),
                    elapsed.as_millis();
                    "caller" => ?caller
                );
            }
        }
    }
}
