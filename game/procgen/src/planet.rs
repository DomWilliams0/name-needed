use crate::continent::ContinentMap;
use crate::rasterize::SlabGrid;
use common::*;

use crate::params::PlanetParams;
use crate::region::{RegionLocation, Regions};
use crate::BlockType;
use std::sync::Arc;
use unit::world::{ChunkLocation, SlabLocation};
use tokio::sync::RwLock;

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet(Arc<RwLock<PlanetInner>>);

unsafe impl Send for Planet {}
unsafe impl Sync for Planet {}

pub struct PlanetInner {
    pub(crate) params: PlanetParams,
    pub(crate) continents: ContinentMap,
    regions: Regions,

    #[cfg(feature = "climate")]
    climate: Option<crate::climate::Climate>,
}

impl Planet {
    // TODO actual error type
    pub fn new(params: PlanetParams) -> BoxedResult<Planet> {
        debug!("creating planet with params {:?}", params);
        let continents = ContinentMap::new(&params);
        let regions = Regions::new(&params);
        let inner = Arc::new(RwLock::new(PlanetInner {
            params,
            continents,
            regions,

            #[cfg(feature = "climate")]
            climate: None,
        }));
        Ok(Self(inner))
    }

    pub async fn initial_generation(&mut self) {
        let planet_ref = self.clone();
        let mut planet = self.0.write().await;
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

        #[cfg(feature = "climate")]
        {
            use crate::climate::*;
            use crate::progress::*;

            let mut progress = match cfg!(feature = "bin") {
                #[cfg(feature = "bin")]
                true if params.render.create_climate_gif => Box::new(
                    GifProgressTracker::new("/tmp/gifs", params.render.gif_threads)
                        .expect("failed to init gif progress tracker"),
                )
                    as Box<dyn ProgressTracker>,

                _ => Box::new(NopProgressTracker) as Box<dyn ProgressTracker>,
            };

            // downgrade planet reference so it can be read from multiple places
            drop(planet);
            let planet = self.0.read().await;

            let climate = Climate::simulate(
                &planet.continents,
                &params,
                &mut planet_rando,
                |step, climate| {
                    progress.update(step, planet_ref.clone(), climate);
                },
            );

            progress.fini();

            // upgrade planet lock again
            drop(planet);
            let mut planet = self.0.write().await;
            planet.climate = Some(climate);
        }
    }

    pub async fn realize_region(&self, region: RegionLocation) {
        let mut inner = self.0.write().await;
        let height_map = inner.continents.generator();
        inner.regions.get_or_create(region, height_map).await;
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

    pub async fn generate_slab(&self, slab: SlabLocation) -> SlabGrid {
        // load region if not already
        let region_loc = RegionLocation::from(slab.chunk);
        self.realize_region(region_loc).await;

        let inner = self.0.write().await;
        let region = inner.regions.get_existing(region_loc).unwrap();
        // let chunk_desc = region.chunk(slab.chunk).description();

        // TODO generate slab

        // TODO rasterize features onto slab

        let mut slab = SlabGrid::default();
        slab[&[0, 0, 0]].ty = BlockType::Stone;
        slab
    }

    /// Instantiate regions and initialize chunks
    pub async fn prepare_for_chunks(&self, (min, max): (ChunkLocation, ChunkLocation)) {
        let regions = (min.0..=max.0)
            .cartesian_product(min.1..=max.1)
            .map(|(cx, cy)| RegionLocation::from(ChunkLocation(cx, cy)))
            .dedup();

        for region in regions {
            self.realize_region(region).await;
        }
    }

    #[cfg(feature = "bin")]
    pub async fn inner(&self) -> impl std::ops::Deref<Target = PlanetInner> + '_ {
        self.0.read().await
    }
}
