use common::{bumpalo::Bump, trace};

/// Arena allocator
pub struct PerFrameStrings {
    arena: Bump,
}

impl PerFrameStrings {
    pub fn new() -> Self {
        Self {
            arena: Bump::with_capacity(4096),
        }
    }

    pub(crate) const fn arena(&self) -> &Bump {
        &self.arena
    }

    pub fn reset(&mut self) {
        trace!(
            "dropping {count} bytes of per frame strings",
            count = self.arena.allocated_bytes()
        );
        self.arena.reset();
    }
}

#[macro_export]
macro_rules! ui_str {
    ( in $ctx:expr, $fmt:expr, $($args:expr),* ) => {{
        use core::fmt::Write;
        let bump = $ctx.arena();
        let mut s = ::common::bumpalo::collections::String::new_in(bump);
        let _ = write!(&mut s, $fmt, $($args),*);
        // safety: s is utf8 from string literal
        unsafe { ::std::str::from_utf8_unchecked(s.into_bump_str().as_bytes()) }
    }};

    ( in $bump:expr, $fmt:expr, $($args:expr,)* ) => {
        $crate::ui_str!(in $bump, $fmt, $($args),*)
    };
}
