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

    pub fn allocator(&self) -> &bumpalo::Bump {
        &self.0
    }
}
