#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct SmallUnsignedConstant(u32);

/// Chunk size X and Y dimension
pub const CHUNK_SIZE: SmallUnsignedConstant = SmallUnsignedConstant(16);

impl SmallUnsignedConstant {
    pub const fn as_f32(self) -> f32 {
        self.0 as f32
    }

    pub const fn as_i32(self) -> i32 {
        self.0 as i32
    }

    pub const fn as_u16(self) -> u16 {
        self.0 as u16
    }

    pub const fn as_i16(self) -> i16 {
        self.0 as i16
    }

    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    pub const fn new(u: u32) -> Self {
        Self(u)
    }
}
