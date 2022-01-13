use common::*;

#[derive(Default)]
pub struct FrameAllocator(bumpalo::Bump);

impl FrameAllocator {
    pub fn reset(&mut self) {
        let bytes = self.0.allocated_bytes();
        if bytes > 0 {
            debug!("freeing {} bytes in frame allocator", bytes);
        }

        self.0.reset();
    }

    pub fn allocator(&self) -> &bumpalo::Bump {
        &self.0
    }
}

// impl Deref for FrameAllocator {
//     type Target = bumpalo::Bump;
//
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }
//
// impl DerefMut for FrameAllocator {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }
