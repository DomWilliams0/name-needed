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
pub use rasterize::{BlockType, GeneratedBlock, SlabGrid};
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

pub use grid_coord_types::{SlabPositionAsCoord, SliceBlockAsCoord};
mod grid_coord_types {
    use common::derive_more::{Deref, From};
    use std::convert::TryFrom;
    use std::fmt::{Debug, Formatter};
    use unit::world::{BlockCoord, LocalSliceIndex, RangePosition, SlabPosition, SliceBlock};

    #[derive(Copy, Clone, Deref, From)]
    pub struct SlabPositionAsCoord(pub SlabPosition);

    #[derive(Copy, Clone, Deref, From)]
    pub struct SliceBlockAsCoord(pub SliceBlock);

    impl grid::CoordType for SlabPositionAsCoord {
        fn try_get(self) -> Option<[usize; 3]> {
            let (x, y, z) = self.0.xyz();
            // assume SlabPosition is correctly constructed and all coords are valid
            Some([x as usize, y as usize, z as usize])
        }

        fn from_coord([x, y, z]: [usize; 3]) -> Option<Self> {
            match (
                BlockCoord::try_from(x),
                BlockCoord::try_from(y),
                i32::try_from(z).ok().and_then(LocalSliceIndex::new),
            ) {
                (Ok(x), Ok(y), Some(z)) => Some(Self(SlabPosition::new_unchecked(x, y, z))),
                _ => None,
            }
        }
    }

    impl grid::CoordType for SliceBlockAsCoord {
        fn try_get(self) -> Option<[usize; 3]> {
            let (x, y) = self.0.xy();
            // assume SliceBlock is correctly constructed and all coords are valid
            Some([usize::from(x), usize::from(y), 0])
        }

        fn from_coord([x, y, _]: [usize; 3]) -> Option<Self> {
            match (BlockCoord::try_from(x), BlockCoord::try_from(y)) {
                (Ok(x), Ok(y)) => SliceBlock::new(x, y).map(Self),
                _ => None,
            }
        }
    }

    impl Debug for SlabPositionAsCoord {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }

    impl Debug for SliceBlockAsCoord {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
}
