#![deny(unused_must_use)]

mod biome;
mod continent;
mod params;
mod planet;
mod rasterize;
mod region;

#[cfg(feature = "bin")]
mod render;

#[cfg(feature = "bin")]
pub use render::Render;

#[cfg(feature = "climate")]
mod progress;

#[cfg(feature = "climate")]
mod climate;

#[cfg(feature = "cache")]
mod cache;

pub use biome::BiomeType;
pub use params::{PlanetParams, PlanetParamsRef};
pub use planet::Planet;
pub use rasterize::{GeneratedBlock, SlabGrid};
pub use region::RegionLocation;

#[cfg(feature = "benchmarking")]
pub mod benchmark_exports {
    pub use super::continent::ContinentMap;
    pub use super::region::{
        RegionChunkUnspecialized, RegionLocationUnspecialized, RegionUnspecialized,
    };
}

/// https://rosettacode.org/wiki/Map_range#Rust
#[inline]
pub(crate) fn map_range<F: common::num_traits::Float>(
    from_range: (F, F),
    to_range: (F, F),
    s: F,
) -> F {
    to_range.0 + (s - from_range.0) * (to_range.1 - to_range.0) / (from_range.1 - from_range.0)
}
