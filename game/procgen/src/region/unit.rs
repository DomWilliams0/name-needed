use unit::world::{BlockPosition, ChunkLocation, RangePosition, WorldPosition, CHUNK_SIZE};

use crate::region::region::CHUNKS_PER_REGION_SIDE;
use crate::region::Region;
use crate::PlanetParams;

/// Is only valid between 0 and planet size, it's the responsibility of the world loader to only
/// request slabs in valid regions
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RegionLocation(pub u32, pub u32);

/// A point on the planet's surface
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct PlanetPoint(f64, f64);

pub struct PlanetPointPrecalculated {
    region: (f64, f64),
    chunk: (i32, i32),
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

impl PlanetPoint {
    const PER_BLOCK: f64 = 1.0 / (CHUNKS_PER_REGION_SIDE.as_f64() * CHUNK_SIZE.as_f64());

    /// If needed for multiple blocks in the same chunk, use [precalculate] and [with_precalculated]
    pub fn from_block(block: WorldPosition) -> Option<Self> {
        let chunk = ChunkLocation::from(block);
        let chunk_idx = Region::chunk_index(chunk);
        let region = RegionLocation::try_from_chunk(chunk)?;

        let precalc = Self::precalculate(region, chunk_idx);
        Some(Self::with_precalculated(&precalc, block.into()))
    }

    pub fn precalculate(region: RegionLocation, chunk_idx: usize) -> PlanetPointPrecalculated {
        let (rx, ry) = (region.0 as f64, region.1 as f64);

        let chunk_idx = chunk_idx as i32;
        let cx = chunk_idx % CHUNKS_PER_REGION_SIDE.as_i32();
        let cy = chunk_idx / CHUNKS_PER_REGION_SIDE.as_i32();
        PlanetPointPrecalculated {
            region: (rx, ry),
            chunk: (cx, cy),
        }
    }

    pub fn with_precalculated(precalc: &PlanetPointPrecalculated, block: BlockPosition) -> Self {
        let (bx, by, _) = BlockPosition::from(block).xyz();
        let &PlanetPointPrecalculated {
            region: (rx, ry),
            chunk: (cx, cy),
        } = precalc;

        let nx = rx + (((cx * CHUNK_SIZE.as_i32()) + bx as i32) as f64 * Self::PER_BLOCK);
        let ny = ry + (((cy * CHUNK_SIZE.as_i32()) + by as i32) as f64 * Self::PER_BLOCK);
        Self(nx, ny)
    }

    #[inline]
    pub const fn get(&self) -> (f64, f64) {
        (self.0, self.1)
    }

    #[inline]
    pub const fn x(&self) -> f64 {
        self.0
    }

    #[inline]
    pub const fn y(&self) -> f64 {
        self.1
    }

    pub const fn new(x: f64, y: f64) -> Self {
        Self(x, y)
    }
}

impl PlanetPointPrecalculated {
    pub const fn chunk(&self) -> (i32, i32) {
        self.chunk
    }
    pub const fn region(&self) -> (f64, f64) {
        self.region
    }
}
