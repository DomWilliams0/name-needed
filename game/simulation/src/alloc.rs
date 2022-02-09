use std::fmt::Write;

use common::*;

#[derive(Default)]
pub struct FrameAllocator(bumpalo::Bump);

impl FrameAllocator {
    pub fn reset(&mut self) {
        let bytes = self.0.allocated_bytes();
        if bytes > 0 {
            trace!("freeing {} bytes in frame allocator", bytes);
            self.0.reset();
        }
    }

    pub fn alloc_str_from_debug(&self, thing: &dyn Debug) -> BumpString {
        let mut s = BumpString::new_in(&self.0);
        let _ = write!(&mut s, "{:?}", thing);
        s
    }

    pub fn alloc_str_from_display(&self, thing: &dyn Display) -> BumpString {
        let mut s = BumpString::new_in(&self.0);
        let _ = write!(&mut s, "{}", thing);
        s
    }

    pub fn allocator(&self) -> &bumpalo::Bump {
        &self.0
    }
}
