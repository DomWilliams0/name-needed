pub mod world {
    use std::ops::{Add, AddAssign, Sub, SubAssign};

    /// A slice of blocks in a chunk, z coordinate
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct SliceIndex(pub SliceIndexType);
    pub type SliceIndexType = i32;

    /// A block in a chunk, x/y coordinate. Must be < chunk size
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct BlockCoord(pub u16);

    /// A block in a chunk
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct Block(pub BlockCoord, pub BlockCoord, pub SliceIndex);

    /// A block in a slice
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct SliceBlock(pub BlockCoord, pub BlockCoord);

    /// A point anywhere in a chunk, x and y > 0 and < chunk size
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct ChunkPoint(pub f32, pub f32, pub f32);

    /// A chunk in the world
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct ChunkPosition(pub i32, pub i32);

    /// A point anywhere in the world
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct WorldPoint(pub f64, pub f64, pub f64);

    // --------
    impl SliceIndex {
        pub const MIN: SliceIndex = Self(std::i32::MIN);
        pub const MAX: SliceIndex = Self(std::i32::MAX);
    }

    // --------
    impl From<u16> for BlockCoord {
        fn from(u: u16) -> Self {
            Self(u)
        }
    }
    impl From<(u16, u16, SliceIndexType)> for Block {
        fn from(pos: (u16, u16, i32)) -> Self {
            let (x, y, z) = pos;
            Self(x.into(), y.into(), SliceIndex(z))
        }
    }
    impl From<SliceIndexType> for SliceIndex {
        fn from(i: i32) -> Self {
            Self(i)
        }
    }
    impl From<(i32, i32)> for ChunkPosition {
        fn from(pos: (i32, i32)) -> Self {
            let (x, y) = pos;
            Self(x, y)
        }
    }

    impl Add<SliceIndexType> for SliceIndex {
        type Output = SliceIndex;

        fn add(self, rhs: i32) -> Self::Output {
            SliceIndex(self.0 + rhs)
        }
    }

    impl AddAssign<SliceIndexType> for SliceIndex {
        fn add_assign(&mut self, rhs: SliceIndexType) {
            self.0 += rhs;
        }
    }

    impl Sub<SliceIndexType> for SliceIndex {
        type Output = SliceIndex;

        fn sub(self, rhs: i32) -> Self::Output {
            SliceIndex(self.0 - rhs)
        }
    }

    impl SubAssign<SliceIndexType> for SliceIndex {
        fn sub_assign(&mut self, rhs: SliceIndexType) {
            self.0 -= rhs;
        }
    }

    impl From<(u16, u16)> for SliceBlock {
        fn from((x, y): (u16, u16)) -> Self {
            Self(BlockCoord(x), BlockCoord(y))
        }
    }
}

pub mod screen {
    /// A point on the screen
    pub struct ScreenPoint(pub u32, pub u32);
}
