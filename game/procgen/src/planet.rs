use common::parking_lot::RwLock;
use common::*;
use grid::{grid_declare, DynamicGrid, GridImpl};
use noise::{Fbm, MultiFractal, NoiseFn, Seedable};
use std::f64::consts::{PI, TAU};
use std::ops::Deref;
use std::sync::Arc;
use unit::world::{ChunkLocation, SlabLocation};
use unit::world::{CHUNK_SIZE, SLAB_SIZE};

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet(Arc<RwLock<PlanetInner>>);

#[derive(Default)]
pub struct Region {
    pub height: f64,
}

pub struct PlanetInner {
    params: PlanetParams,
    regions: DynamicGrid<Region>,
}

#[derive(Debug, Clone)]
pub struct PlanetParams {
    pub seed: u64,
    pub radius: u32,

    /// Dims of raw region grid
    pub planet_size: usize,
}

// TODO custom block types for procgen that are translated to game blocks
#[derive(Clone, Default, Debug, Copy)]
pub struct GeneratedBlock {
    // cant be zst apparently
    dummy: i32,
}

// redeclaration of slab grid
// TODO move this
grid_declare!(pub struct SlabGrid<SlabGridImpl, GeneratedBlock>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

impl Default for PlanetParams {
    fn default() -> Self {
        PlanetParams {
            seed: 0,
            radius: 4,
            planet_size: 8,
        }
    }
}

/// https://rosettacode.org/wiki/Map_range#Rust
#[inline]
fn map_range(from_range: (f64, f64), to_range: (f64, f64), s: f64) -> f64 {
    to_range.0 + (s - from_range.0) * (to_range.1 - to_range.0) / (from_range.1 - from_range.0)
}

impl Planet {
    // TODO actual error type
    pub fn new(params: PlanetParams) -> Result<Planet, &'static str> {
        let regions = {
            let sz = params.planet_size;
            DynamicGrid::<Region>::new((sz, sz, 1))
        };
        debug!("creating planet with params {:?}", params);
        let inner = Arc::new(RwLock::new(PlanetInner { params, regions }));
        Ok(Self(inner))
    }

    pub fn initial_generation(&mut self) {
        let mut planet = self.0.write();
        let params = planet.params.clone();

        // populate heightmap
        let noise = Fbm::new()
            .set_seed(params.seed as u32) // TODO seed loses half its entropy
            .set_octaves(5)
            .set_frequency(0.2)
            // .set_lacunarity(0.4)
            // .set_persistence(0.45)
        ;

        let sz = params.planet_size;
        let mut i = 0;
        for ry in 0..sz {
            for rx in 0..sz {
                let scale = 4.0;
                let nx = rx as f64 / scale;
                let ny = ry as f64 / scale;
                let val = noise.get([nx, ny, 0.6]);

                let height = map_range((-1.0, 1.0), (0.0, 1.0), val);
                planet.regions.index_mut(i).height = height;
                i += 1;
            }
        }
    }

    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        // TODO could have separate copy of planet params per thread if immutable
        let inner = self.0.read();

        // radius is excluding 0,0
        let radius = inner.params.radius as i32;
        (
            ChunkLocation(-radius, -radius),
            ChunkLocation(radius, radius),
        )
    }

    pub fn generate_slab(&self, slab: SlabLocation) -> SlabGrid {
        SlabGrid::default()
    }

    #[cfg(feature = "bin")]
    pub fn inner(&self) -> impl Deref<Target = PlanetInner> + '_ {
        self.0.read()
    }
}

impl PlanetInner {
    pub fn regions(&self) -> &DynamicGrid<Region> {
        &self.regions
    }
}
