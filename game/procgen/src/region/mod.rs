#![deny(unused_must_use)]
#![allow(dead_code)]

pub(crate) use feature::generate_loose_subfeatures;
pub use feature::{ApplyFeatureContext, Feature, RegionalFeature};
pub(crate) use subfeature::SlabContinuation;

mod feature;
mod features;
#[allow(clippy::module_inception)]
mod region;
mod regions;
mod row_scanning;
mod subfeature;
mod subfeatures;
mod unit;

/// Each region is broken up into this many chunks per side, i.e. this^2 for total number of chunks
pub const CHUNKS_PER_REGION_SIDE: usize = 8;
pub const CHUNKS_PER_REGION: usize = CHUNKS_PER_REGION_SIDE * CHUNKS_PER_REGION_SIDE;

// specialize region types for the defined planet size

pub type Region = region::Region<CHUNKS_PER_REGION_SIDE, CHUNKS_PER_REGION>;
pub type Regions = regions::Regions<CHUNKS_PER_REGION_SIDE, CHUNKS_PER_REGION>;
pub type RegionLocation = unit::RegionLocation<CHUNKS_PER_REGION_SIDE>;
pub type PlanetPoint = unit::PlanetPoint<CHUNKS_PER_REGION_SIDE>;
pub(crate) type LoadedRegionRef<'a> =
    regions::LoadedRegionRef<'a, CHUNKS_PER_REGION_SIDE, CHUNKS_PER_REGION>;

pub type RegionLocationUnspecialized<const SIZE: usize> = unit::RegionLocation<SIZE>;
pub type RegionUnspecialized<const SIZE: usize, const SIZE_2: usize> = region::Region<SIZE, SIZE_2>;
pub type RegionChunkUnspecialized<const SIZE: usize> = region::RegionChunk<SIZE>;

common::slog_kv_debug!(RegionLocation, "region");
common::slog_value_debug!(RegionLocation);
