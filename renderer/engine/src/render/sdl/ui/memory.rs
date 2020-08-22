use common::{bumpalo::Bump, trace};

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
    ( in $bump:expr, $fmt:expr, $($args:expr),* ) => {{
        use core::fmt::Write;
        let bump = $bump.arena();
        let mut s = ::common::bumpalo::collections::String::new_in(bump);
        let _ = write!(&mut s, concat!($fmt, "\0"), $($args),*);
        // safety: s is nul terminated utf8
        unsafe { ::imgui::ImStr::from_utf8_with_nul_unchecked(s.into_bump_str().as_bytes()) }
    }};

    ( in $bump:expr, $fmt:expr, $($args:expr,)* ) => {
        $crate::ui_str!(in $bump, $fmt, $($args),*)
    };
}
