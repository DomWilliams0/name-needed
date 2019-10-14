pub mod world {
    use std::i32;
    use std::ops::{Add, AddAssign, Sub, SubAssign};

    use super::dim::CHUNK_SIZE;
    use crate::grid::CoordType;
    use std::fmt::{Display, Error, Formatter};

    /// A slice of blocks in a chunk, z coordinate
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct SliceIndex(pub SliceIndexType);
    pub type SliceIndexType = i32;

    /// A block in a chunk, x/y coordinate. Must be < chunk size
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct BlockCoord(pub u16);

    /// A block in a chunk
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct BlockPosition(pub BlockCoord, pub BlockCoord, pub SliceIndex);

    /// A block in a slice
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct SliceBlock(pub BlockCoord, pub BlockCoord);

    /// A point anywhere in a chunk, x and y > 0 and < chunk size
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct ChunkPoint(pub f32, pub f32, pub f32);

    /// A chunk in the world
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
    pub struct ChunkPosition(pub i32, pub i32);

    /// A point anywhere in the world
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct WorldPoint(pub f32, pub f32, pub f32);

    /// A block anywhere in the world
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct WorldPosition(pub i32, pub i32, pub i32);

    // --------
    impl SliceIndex {
        pub const MIN: SliceIndex = Self(std::i32::MIN);
        pub const MAX: SliceIndex = Self(std::i32::MAX);

        pub fn abs(self) -> Self {
            Self(self.0.abs())
        }
    }

    impl BlockPosition {
        pub fn to_world_pos<P: Into<ChunkPosition>>(self, chunk_pos: P) -> WorldPosition {
            let ChunkPosition(cx, cy) = chunk_pos.into();
            let BlockPosition(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = self;
            WorldPosition(
                i32::from(x) + cx * CHUNK_SIZE.as_i32(),
                i32::from(y) + cy * CHUNK_SIZE.as_i32(),
                z,
            )
        }
        pub fn to_world_point<P: Into<ChunkPosition>>(self, chunk_pos: P) -> WorldPoint {
            let WorldPosition(x, y, z) = self.to_world_pos(chunk_pos);
            WorldPoint(x as f32, y as f32, z as f32)
        }

        pub fn to_world_point_centered<P: Into<ChunkPosition>>(self, chunk_pos: P) -> WorldPoint {
            let WorldPoint(x, y, z) = self.to_world_point(chunk_pos);
            WorldPoint(x + 0.5, y + 0.5, z)
        }

        pub fn flatten(self) -> (u16, u16, SliceIndexType) {
            self.into()
        }

        pub fn to_chunk_point_centered(self) -> ChunkPoint {
            let BlockPosition(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = self;
            ChunkPoint(f32::from(x), f32::from(y), z as f32)
        }
    }

    impl Display for BlockPosition {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            write!(
                f,
                "BlockPosition({}, {}, {})",
                (self.0).0,
                (self.1).0,
                (self.2).0
            )
        }
    }

    impl Display for WorldPosition {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            write!(f, "WorldPosition({}, {}, {})", self.0, self.1, self.2)
        }
    }

    impl SliceBlock {
        pub fn to_block_position(self, slice: SliceIndex) -> BlockPosition {
            BlockPosition(self.0, self.1, slice)
        }
    }

    // --------
    impl From<u16> for BlockCoord {
        fn from(u: u16) -> Self {
            Self(u)
        }
    }

    impl From<(u16, u16, SliceIndexType)> for BlockPosition {
        fn from(pos: (u16, u16, i32)) -> Self {
            let (x, y, z) = pos;
            Self(x.into(), y.into(), SliceIndex(z))
        }
    }

    impl From<(i32, i32, i32)> for BlockPosition {
        fn from(pos: (i32, i32, i32)) -> Self {
            let (x, y, z) = pos;
            Self((x as u16).into(), (y as u16).into(), z.into())
        }
    }

    impl From<&CoordType> for BlockPosition {
        fn from(pos: &CoordType) -> Self {
            let &[x, y, z] = pos;
            Self((x as u16).into(), (y as u16).into(), SliceIndex(z))
        }
    }

    impl From<BlockPosition> for CoordType {
        fn from(b: BlockPosition) -> Self {
            let BlockPosition(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = b;
            [i32::from(x), i32::from(y), z]
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

    impl From<BlockPosition> for SliceBlock {
        fn from(b: BlockPosition) -> Self {
            Self(b.0, b.1)
        }
    }

    impl From<BlockPosition> for (u16, u16, SliceIndexType) {
        fn from(b: BlockPosition) -> Self {
            let BlockPosition(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = b;
            (x, y, z)
        }
    }

    impl From<(f32, f32, f32)> for WorldPoint {
        fn from((x, y, z): (f32, f32, f32)) -> Self {
            WorldPoint(x, y, z)
        }
    }

    impl From<ChunkPoint> for cgmath::Vector3<f32> {
        fn from(p: ChunkPoint) -> Self {
            let ChunkPoint(x, y, z) = p;
            cgmath::Vector3 { x, y, z }
        }
    }

    impl From<ChunkPosition> for WorldPoint {
        fn from(p: ChunkPosition) -> Self {
            Self(
                (p.0 * CHUNK_SIZE.as_i32()) as f32,
                (p.1 * CHUNK_SIZE.as_i32()) as f32,
                0.0,
            )
        }
    }

    impl From<WorldPoint> for [f32; 3] {
        fn from(p: WorldPoint) -> Self {
            let WorldPoint(x, y, z) = p;
            [x, y, z]
        }
    }

    impl From<(i32, i32, i32)> for WorldPosition {
        fn from((x, y, z): (i32, i32, i32)) -> Self {
            Self(x, y, z)
        }
    }
}

pub mod screen {
    /// A point on the screen
    pub struct ScreenPoint(pub u32, pub u32);
}

pub mod dim {
    /// Chunk size X and Y dimension
    #[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
    pub struct ChunkDimension(u32);

    pub const CHUNK_SIZE: ChunkDimension = ChunkDimension(16);

    // TODO temporary while chunks are still fixed 3d grids (#15)
    pub const CHUNK_DEPTH: ChunkDimension = ChunkDimension(16);

    impl ChunkDimension {
        pub const fn as_f32(self) -> f32 {
            self.0 as f32
        }

        pub const fn as_i32(self) -> i32 {
            self.0 as i32
        }

        pub const fn as_u16(self) -> u16 {
            self.0 as u16
        }

        pub const fn as_usize(self) -> usize {
            self.0 as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::EPSILON;

    use float_cmp::ApproxEq;

    use crate::coordinate::dim::CHUNK_SIZE;
    use crate::coordinate::world::{BlockCoord, BlockPosition, SliceIndex};
    use crate::WorldPoint;

    #[test]
    fn block_to_world() {
        // ensure block positions convert to the expected world position
        let b = BlockPosition(BlockCoord(1), BlockCoord(2), SliceIndex(3));

        // at origin
        let WorldPoint(x, y, z) = b.to_world_point((0, 0));
        assert!(x.approx_eq(1.0, (EPSILON, 2)));
        assert!(y.approx_eq(2.0, (EPSILON, 2)));
        assert!(z.approx_eq(3.0, (EPSILON, 2)));

        // a few chunks over
        let WorldPoint(x, y, z) = b.to_world_point((1, 2));
        let sz: f32 = CHUNK_SIZE.as_f32();
        assert!(x.approx_eq(1.0 + sz, (EPSILON, 2)));
        assert!(y.approx_eq(2.0 + sz + sz, (EPSILON, 2)));
        assert!(z.approx_eq(3.0, (EPSILON, 2)));
    }

    #[test]
    fn negative_block_to_world() {
        // negative chunk coords should be handled fine
        let b: BlockPosition = (0, 0, 0).into();
        let wp = b.to_world_point((-1, -1));
        assert_eq!(
            wp,
            WorldPoint(-CHUNK_SIZE.as_f32(), -CHUNK_SIZE.as_f32(), 0.0)
        );
    }
}
