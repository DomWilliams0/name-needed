mod climate;
mod continent;
mod params;
mod planet;
mod progress;
mod rasterize;

#[cfg(feature = "bin")]
mod render;

#[cfg(feature = "bin")]
pub use render::Render;

pub use params::PlanetParams;
pub use planet::Planet;
pub use rasterize::SlabGrid;

/// https://rosettacode.org/wiki/Map_range#Rust
#[inline]
pub(crate) fn map_range<F: common::num_traits::Float>(
    from_range: (F, F),
    to_range: (F, F),
    s: F,
) -> F {
    to_range.0 + (s - from_range.0) * (to_range.1 - to_range.0) / (from_range.1 - from_range.0)
}
