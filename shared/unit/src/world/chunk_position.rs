use derive_more::*;

use crate::dim::CHUNK_SIZE;
use crate::world::WorldPosition;

/// A chunk in the world
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Into, From)]
pub struct ChunkPosition(pub i32, pub i32);

impl From<WorldPosition> for ChunkPosition {
    fn from(wp: WorldPosition) -> Self {
        let WorldPosition(x, y, _) = wp;
        ChunkPosition(
            x.div_euclid(CHUNK_SIZE.as_i32()),
            y.div_euclid(CHUNK_SIZE.as_i32()),
        )
    }
}
