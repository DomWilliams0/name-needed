mod feature;
mod features;
mod region;
mod row_scanning;
mod subfeature;
mod subfeatures;
mod unit;

pub use feature::{ApplyFeatureContext, Feature, RegionalFeature};

/// Each region is broken up into this many chunks per side, i.e. this^2 for total number of chunks
pub const CHUNKS_PER_REGION_SIDE: usize = 8;
pub const CHUNKS_PER_REGION: usize = CHUNKS_PER_REGION_SIDE * CHUNKS_PER_REGION_SIDE;

// specialize region types for the defined planet size

pub type Region = region::Region<CHUNKS_PER_REGION_SIDE, CHUNKS_PER_REGION>;
pub type Regions = region::Regions<CHUNKS_PER_REGION_SIDE, CHUNKS_PER_REGION>;
pub type RegionLocation = unit::RegionLocation<CHUNKS_PER_REGION_SIDE>;
pub type PlanetPoint = unit::PlanetPoint<CHUNKS_PER_REGION_SIDE>;

pub type RegionLocationUnspecialized<const SIZE: usize> = unit::RegionLocation<SIZE>;
pub type RegionUnspecialized<const SIZE: usize, const SIZE_2: usize> = region::Region<SIZE, SIZE_2>;
pub type RegionChunkUnspecialized<const SIZE: usize> = region::RegionChunk<SIZE>;
