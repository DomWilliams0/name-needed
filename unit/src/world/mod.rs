pub use self::grid::*;
pub use block_position::*;
pub use chunk_location::*;
pub use range::*;
pub use slab_index::*;
pub use slab_location::*;
pub use slab_position::*;
pub use slice_block::*;
pub use slice_index::*;
pub use world_point::*;
pub use world_position::*;

use crate::dim::SmallUnsignedConstant;

/// 3x3x3 blocks per 1m^3
pub const BLOCKS_PER_METRE: SmallUnsignedConstant = SmallUnsignedConstant::new(3);

pub const BLOCKS_SCALE: f32 = 1.0 / BLOCKS_PER_METRE.as_f32();

/// Chunk size X and Y dimension
pub const CHUNK_SIZE: SmallUnsignedConstant = SmallUnsignedConstant::new(16);

/// Chunk size Z dimension
pub const SLAB_SIZE: SmallUnsignedConstant = SmallUnsignedConstant::new(32);

mod block_position;
mod chunk_location;
mod grid;
mod range;
mod slab_index;
mod slab_location;
mod slab_position;
mod slice_block;
mod slice_index;
mod world_point;
mod world_position;

#[cfg(test)]
mod tests {

    use misc::*;

    use crate::world::{
        BlockPosition, ChunkLocation, GlobalSliceIndex, WorldPoint, WorldPosition, CHUNK_SIZE,
    };

    #[test]
    fn block_to_world() {
        // ensure block positions convert to the expected world position
        let b = BlockPosition::new_unchecked(1, 2, GlobalSliceIndex::new(3));

        // at origin
        let (x, y, z) = b.to_world_point((0, 0)).xyz();
        assert!(x.approx_eq(1.0, (f32::EPSILON, 2)));
        assert!(y.approx_eq(2.0, (f32::EPSILON, 2)));
        assert!(z.approx_eq(3.0, (f32::EPSILON, 2)));

        // a few chunks over
        let (x, y, z) = b.to_world_point((1, 2)).xyz();
        let sz: f32 = CHUNK_SIZE.as_f32();
        assert!(x.approx_eq(1.0 + sz, (f32::EPSILON, 2)));
        assert!(y.approx_eq(2.0 + sz + sz, (f32::EPSILON, 2)));
        assert!(z.approx_eq(3.0, (f32::EPSILON, 2)));
    }

    #[test]
    fn negative_block_to_world() {
        // negative chunk coords should be handled fine
        let b: BlockPosition = BlockPosition::new_unchecked(0, 0, GlobalSliceIndex::new(0));
        let wp = b.to_world_point((-1, -1));
        assert_eq!(
            wp,
            WorldPoint::new_unchecked(-CHUNK_SIZE.as_f32(), -CHUNK_SIZE.as_f32(), 0.0)
        );
    }

    #[test]
    fn world_to_chunk() {
        assert_eq!(
            ChunkLocation::from(WorldPosition(10, 20, GlobalSliceIndex::new(50))),
            ChunkLocation(0, 1)
        );
        assert_eq!(
            ChunkLocation::from(WorldPosition(-20, -40, GlobalSliceIndex::new(50))),
            ChunkLocation(-2, -3)
        );

        assert_eq!(
            ChunkLocation::from(WorldPosition(-2, 2, GlobalSliceIndex::new(0))),
            ChunkLocation(-1, 0)
        );
    }

    #[test]
    fn negative_world_to_block() {
        assert_eq!(
            BlockPosition::from(WorldPosition(-10, -10, GlobalSliceIndex::new(-10))),
            BlockPosition::new_unchecked(6, 6, GlobalSliceIndex::new(-10))
        );
    }
}
