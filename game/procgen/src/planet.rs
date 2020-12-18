use crate::continent::ContinentMap;
use crate::rasterize::SlabGrid;
use common::parking_lot::RwLock;
use common::*;

use crate::params::PlanetParams;
use std::sync::Arc;
use unit::world::{ChunkLocation, SlabLocation};

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet(Arc<RwLock<PlanetInner>>);

unsafe impl Send for Planet {}
unsafe impl Sync for Planet {}

#[derive(Default)]
pub struct Region {
    pub height: f64,
}

pub struct PlanetInner {
    pub(crate) params: PlanetParams,
    pub(crate) continents: ContinentMap,
}

impl Planet {
    // TODO actual error type
    pub fn new(params: PlanetParams) -> Result<Planet, &'static str> {
        debug!("creating planet with params {:?}", params);
        let continents = ContinentMap::new(&params);
        let inner = Arc::new(RwLock::new(PlanetInner { params, continents }));
        Ok(Self(inner))
    }

    pub fn initial_generation(&mut self) {
        let mut planet = self.0.write();
        let params = planet.params.clone();

        let mut planet_rando = StdRng::seed_from_u64(params.seed());

        // place continents
        let (continents, total_blobs) = planet.continents.generate(&mut planet_rando);
        // TODO reject if continent or land blob count is too low
        info!(
            "placed {count} continents with {blobs} land blobs",
            count = continents,
            blobs = total_blobs
        );

        // rasterize continents onto grid and discover depth i.e. distance from land/sea border,
        // and place initial heightmap
        planet.continents.discover(&mut planet_rando);
    }

    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        // TODO could have separate copy of planet params per thread if immutable

        // radius is excluding 0,0
        // TODO radius no longer makes sense
        let radius = 5;
        (
            ChunkLocation(-radius, -radius),
            ChunkLocation(radius, radius),
        )
    }

    pub fn generate_slab(&self, slab: SlabLocation) -> SlabGrid {
        SlabGrid::default()
    }

    #[cfg(feature = "bin")]
    pub fn inner(&self) -> impl std::ops::Deref<Target = PlanetInner> + '_ {
        self.0.read()
    }
}
