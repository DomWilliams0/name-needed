use unit::world::{BlockPosition, ChunkLocation, RangePosition, WorldPosition, CHUNK_SIZE};

use crate::region::region::CHUNKS_PER_REGION_SIDE;
use crate::region::Region;
use crate::PlanetParams;

/// Is only valid between 0 and planet size, it's the responsibility of the world loader to only
/// request slabs in valid regions
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RegionLocation(pub u32, pub u32);

pub const PER_BLOCK: f64 = 1.0 / (CHUNKS_PER_REGION_SIDE.as_f64() * CHUNK_SIZE.as_f64());

pub fn noise_pos_for_block(block: WorldPosition) -> Option<(f64, f64)> {
    let chunk = ChunkLocation::from(block);
    let chunk_idx = Region::chunk_index(chunk);
    let region = RegionLocation::try_from_chunk(chunk)?;
    let (rx, ry) = (region.0 as f64, region.1 as f64);

    let chunk_idx = chunk_idx as i32;
    let cx = chunk_idx % CHUNKS_PER_REGION_SIDE.as_i32();
    let cy = chunk_idx / CHUNKS_PER_REGION_SIDE.as_i32();

    let (bx, by, _) = BlockPosition::from(block).xyz();

    let nx = rx + (((cx * CHUNK_SIZE.as_i32()) + bx as i32) as f64 * PER_BLOCK);
    let ny = ry + (((cy * CHUNK_SIZE.as_i32()) + by as i32) as f64 * PER_BLOCK);
    Some((nx, ny))
}

impl RegionLocation {
    /// None if negative
    pub fn try_from_chunk(chunk: ChunkLocation) -> Option<Self> {
        let x = chunk.0.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let y = chunk.1.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());

        if x >= 0 && y >= 0 {
            Some(RegionLocation(x as u32, y as u32))
        } else {
            None
        }
    }

    /// None if negative or greater than planet size
    pub fn try_from_chunk_with_params(chunk: ChunkLocation, params: &PlanetParams) -> Option<Self> {
        let x = chunk.0.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let y = chunk.1.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let limit = 0..params.planet_size as i32;

        if limit.contains(&x) && limit.contains(&y) {
            Some(RegionLocation(x as u32, y as u32))
        } else {
            None
        }
    }

    /// Inclusive bounds
    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        let x = self.0 as i32;
        let y = self.1 as i32;

        let min = (
            x * CHUNKS_PER_REGION_SIDE.as_i32(),
            y * CHUNKS_PER_REGION_SIDE.as_i32(),
        );
        let max = (
            min.0 + CHUNKS_PER_REGION_SIDE.as_i32() - 1,
            min.1 + CHUNKS_PER_REGION_SIDE.as_i32() - 1,
        );
        (min.into(), max.into())
    }

    pub fn local_chunk_to_global(&self, local_chunk: ChunkLocation) -> ChunkLocation {
        assert!((0..CHUNKS_PER_REGION_SIDE.as_i32()).contains(&local_chunk.x()));
        assert!((0..CHUNKS_PER_REGION_SIDE.as_i32()).contains(&local_chunk.y()));

        ChunkLocation(
            (self.0 as i32 * CHUNKS_PER_REGION_SIDE.as_i32()) + local_chunk.x(),
            (self.1 as i32 * CHUNKS_PER_REGION_SIDE.as_i32()) + local_chunk.y(),
        )
    }
}
