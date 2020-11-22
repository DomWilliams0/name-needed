pub use block_position::*;
pub use chunk_location::*;
pub use chunk_point::*;
pub use range::*;
pub use slab_index::*;
pub use slab_location::*;
pub use slab_position::*;
pub use slice_block::*;
pub use slice_index::*;
pub use world_point::*;
pub use world_position::*;

mod block_position;
mod chunk_location;
mod chunk_point;
mod range;
mod slab_index;
mod slab_location;
mod slab_position;
mod slice_block;
mod slice_index;
mod world_point;
mod world_position;

// TODO overhaul all *Position and *Point to impl common traits, to reduce repeated code and From/Intos

/// 2 blocks per 1m
pub const SCALE: f32 = 0.5;

#[cfg(test)]
mod tests {
    use std::f32::EPSILON;

    use common::*;

    use crate::dim::CHUNK_SIZE;
    use crate::world::{BlockPosition, ChunkLocation, SliceIndex, WorldPoint, WorldPosition};

    #[test]
    fn block_to_world() {
        // ensure block positions convert to the expected world position
        let b = BlockPosition::new(1, 2, SliceIndex::new(3));

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

    #[test]
    fn world_to_chunk() {
        assert_eq!(
            ChunkLocation::from(WorldPosition(10, 20, SliceIndex::new(50))),
            ChunkLocation(0, 1)
        );
        assert_eq!(
            ChunkLocation::from(WorldPosition(-20, -40, SliceIndex::new(50))),
            ChunkLocation(-2, -3)
        );

        assert_eq!(
            ChunkLocation::from(WorldPosition(-2, 2, SliceIndex::new(0))),
            ChunkLocation(-1, 0)
        );
    }

    #[test]
    fn negative_world_to_block() {
        assert_eq!(
            BlockPosition::from(WorldPosition(-10, -10, SliceIndex::new(-10))),
            BlockPosition::from((6, 6, SliceIndex::new(-10)))
        );
    }
}
