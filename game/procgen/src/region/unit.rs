use unit::world::{BlockPosition, ChunkLocation, GlobalSliceIndex, WorldPosition, CHUNK_SIZE};

use crate::region::Region;
use crate::PlanetParams;

/// Is only valid between 0 and planet size, it's the responsibility of the world loader to only
/// request slabs in valid regions.
///
/// SIZE param = chunks per region side
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RegionLocation<const SIZE: usize>(u32, u32);

/// A point on the planet's surface
///
/// SIZE param = chunks per region side
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct PlanetPoint<const SIZE: usize>(f64, f64);

pub struct PlanetPointPrecalculated {
    region: (f64, f64),
    chunk: (i32, i32),
}

impl<const SIZE: usize> RegionLocation<SIZE> {
    pub const fn new(x: u32, y: u32) -> Self {
        Self(x, y)
    }

    /// None if negative
    pub fn try_from_chunk(chunk: ChunkLocation) -> Option<Self> {
        let x = chunk.0.div_euclid(SIZE as i32);
        let y = chunk.1.div_euclid(SIZE as i32);

        if x >= 0 && y >= 0 {
            Some(RegionLocation(x as u32, y as u32))
        } else {
            None
        }
    }

    /// None if negative or greater than planet size
    pub fn try_from_chunk_with_params(chunk: ChunkLocation, params: &PlanetParams) -> Option<Self> {
        let x = chunk.0.div_euclid(SIZE as i32);
        let y = chunk.1.div_euclid(SIZE as i32);
        let limit = 0..params.planet_size as i32;

        if limit.contains(&x) && limit.contains(&y) {
            Some(RegionLocation(x as u32, y as u32))
        } else {
            None
        }
    }

    /// None if (self+offset) is negative or greater than planet size
    pub fn try_add_offset_with_params(
        &self,
        offset: (i32, i32),
        params: &PlanetParams,
    ) -> Option<Self> {
        let (x, y) = (self.0 as i32 + offset.0, self.1 as i32 + offset.1);

        let limit = 0..params.planet_size as i32;
        if x >= 0 && y >= 0 && limit.contains(&x) && limit.contains(&y) {
            Some(RegionLocation(x as u32, y as u32))
        } else {
            None
        }
    }

    /// Inclusive bounds
    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        let x = self.0 as i32;
        let y = self.1 as i32;

        let min = (x * SIZE as i32, y * SIZE as i32);
        let max = (min.0 + SIZE as i32 - 1, min.1 + SIZE as i32 - 1);
        (min.into(), max.into())
    }

    pub fn local_chunk_to_global(&self, local_chunk: ChunkLocation) -> ChunkLocation {
        assert!((0..SIZE as i32).contains(&local_chunk.x()));
        assert!((0..SIZE as i32).contains(&local_chunk.y()));

        ChunkLocation(
            (self.0 as i32 * SIZE as i32) + local_chunk.x(),
            (self.1 as i32 * SIZE as i32) + local_chunk.y(),
        )
    }

    pub const fn xy(self) -> (u32, u32) {
        (self.0, self.1)
    }

    pub const fn xy_f(self) -> (f64, f64) {
        (self.0 as f64, self.1 as f64)
    }
}

impl<const SIZE: usize> PlanetPoint<SIZE> {
    pub const PER_BLOCK: f64 = 1.0 / (SIZE as f64 * CHUNK_SIZE.as_f64());

    /// If needed for multiple blocks in the same chunk, use [precalculate] and [with_precalculated]
    pub fn from_block(block: WorldPosition) -> Option<Self> {
        let chunk = ChunkLocation::from(block);
        let chunk_idx = Region::chunk_index(chunk);
        let region = RegionLocation::try_from_chunk(chunk)?;

        let precalc = Self::precalculate(region, chunk_idx);
        Some(Self::with_precalculated(&precalc, block.into()))
    }

    pub fn precalculate(
        region: RegionLocation<SIZE>,
        chunk_idx: usize,
    ) -> PlanetPointPrecalculated {
        let (rx, ry) = (region.0 as f64, region.1 as f64);

        let chunk_idx = chunk_idx as i32;
        let cx = chunk_idx % SIZE as i32;
        let cy = chunk_idx / SIZE as i32;
        PlanetPointPrecalculated {
            region: (rx, ry),
            chunk: (cx, cy),
        }
    }

    pub fn with_precalculated(precalc: &PlanetPointPrecalculated, block: BlockPosition) -> Self {
        let (bx, by, _) = block.flatten();
        let &PlanetPointPrecalculated {
            region: (rx, ry),
            chunk: (cx, cy),
        } = precalc;

        let nx = rx + (((cx * CHUNK_SIZE.as_i32()) + bx as i32) as f64 * Self::PER_BLOCK);
        let ny = ry + (((cy * CHUNK_SIZE.as_i32()) + by as i32) as f64 * Self::PER_BLOCK);
        Self(nx, ny)
    }

    pub fn into_block(self, z: GlobalSliceIndex) -> WorldPosition {
        let (x, y) = self.get();
        WorldPosition::from((
            (x / Self::PER_BLOCK) as i32,
            (y / Self::PER_BLOCK) as i32,
            z,
        ))
    }

    #[inline]
    pub const fn get(&self) -> (f64, f64) {
        (self.0, self.1)
    }

    #[inline]
    pub const fn get_array(&self) -> [f64; 2] {
        [self.0, self.1]
    }

    #[inline]
    pub const fn x(&self) -> f64 {
        self.0
    }

    #[inline]
    pub const fn y(&self) -> f64 {
        self.1
    }

    /// Ensure scale is right!
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

impl<const SIZE: usize> From<[f64; 2]> for PlanetPoint<SIZE> {
    #[inline]
    fn from([x, y]: [f64; 2]) -> Self {
        Self(x, y)
    }
}
