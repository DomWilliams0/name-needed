use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use common::*;
use unit::world::{
    BlockPosition, ChunkLocation, GlobalSliceIndex, SlabIndex, SlabLocation, SlabPosition,
    SliceIndex, WorldPosition, CHUNK_SIZE,
};

use crate::biome::BlockQueryResult;
use crate::continent::ContinentMap;
use crate::params::PlanetParamsRef;
use crate::rasterize::SlabGrid;
use crate::region::{generate_loose_subfeatures, Regions};
use crate::region::{
    ApplyFeatureContext, LoadedRegionRef, PlanetPoint, RegionLocation, SlabContinuation,
};

use crate::GeneratedBlock;
use geo::{Coordinate, Rect};
use world_types::EntityDescription;

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet(Arc<RwLock<PlanetInner>>);

unsafe impl Send for Planet {}
unsafe impl Sync for Planet {}

pub struct PlanetInner {
    pub(crate) params: PlanetParamsRef,
    pub(crate) continents: ContinentMap,
    pub(crate) regions: Regions,

    /// Reused allocation for block updates, queried by the game each tick. These come from
    /// rasterizing subfeatures that protrude into _already loaded_ slabs, and so need to be applied
    /// to the slab post-load
    world_updates: Arc<Mutex<Vec<(WorldPosition, GeneratedBlock)>>>,

    #[cfg(feature = "climate")]
    climate: Option<crate::climate::Climate>,

    #[cfg(feature = "cache")]
    was_loaded: bool,
}

pub struct GeneratedPlanetSlab {
    pub terrain: SlabGrid,
    pub entities: Vec<EntityDescription>,
}

impl Planet {
    // TODO actual error type
    pub fn new(params: PlanetParamsRef) -> BoxedResult<Planet> {
        debug!("creating planet with params {:?}", params);

        let mut continents = None;

        #[cfg(feature = "cache")]
        {
            if !params.no_cache {
                match crate::cache::try_load(&params) {
                    Ok(None) => info!("no cache found, generating from scratch"),
                    Ok(Some(nice)) => {
                        info!("loaded cached planet from disk");
                        continents = Some(nice);
                    }
                    Err(e) => {
                        error!("failed to load planet from cache: {}", e);
                    }
                }
            }
        }

        #[cfg(feature = "cache")]
        let was_loaded = continents.is_some();
        let continents = continents.unwrap_or_else(|| ContinentMap::new(params.clone()));

        let regions = Regions::new(params.clone());
        let inner = Arc::new(RwLock::new(PlanetInner {
            params,
            continents,
            regions,
            world_updates: Arc::new(Mutex::new(Vec::with_capacity(256))),

            #[cfg(feature = "climate")]
            climate: None,

            #[cfg(feature = "cache")]
            was_loaded,
        }));

        Ok(Self(inner))
    }

    pub async fn initial_generation(&mut self) -> BoxedResult<()> {
        let mut planet = self.0.write().await;
        let mut planet_rando = StdRng::seed_from_u64(planet.params.seed());

        // initialize generator unconditionally
        planet.continents.init_generator(&mut planet_rando)?;

        #[cfg(feature = "cache")]
        {
            if planet.was_loaded {
                debug!("skipping generation for planet loaded from cache");
                return Ok(());
            }
        }

        info!("generating planet");
        let params = planet.params.clone();

        // place continents and seed temp/moisture etc
        planet.continents.generate(&mut planet_rando);

        drop(planet);

        #[cfg(feature = "climate")]
        {
            use crate::climate::*;
            use crate::progress::*;

            // let planet_ref = self.clone();
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
            let planet = self.0.read().await;

            let climate = Climate::simulate(
                &planet.continents,
                &params,
                &mut planet_rando,
                |_step, _climate| {
                    // TODO this is a future! the climate feature is incomplete and a wip so gonna leave this broken for now
                    // progress.update(step, planet_ref.clone(), climate);
                    unreachable!("climate is experimental and therefore broken")
                },
            );

            progress.fini();

            // upgrade planet lock again
            drop(planet);
            let mut planet = self.0.write().await;
            planet.climate = Some(climate);
        }

        #[cfg(feature = "cache")]
        if !params.no_cache {
            let planet = self.0.read().await;
            if let Err(e) = crate::cache::save(&planet) {
                error!("failed to serialize planet: {}", e);
            }
        }

        Ok(())
    }

    pub async fn realize_region(&self, region: RegionLocation) {
        let inner = self.0.read().await;
        inner.get_or_create_region(region).await;
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

    /// Generates now and does not cache. Returns None if slab is out of range
    pub async fn generate_slab(&self, slab: SlabLocation) -> Option<GeneratedPlanetSlab> {
        let inner = self.0.read().await;
        let params = inner.params.clone();
        let slab_continuations = inner.regions.slab_continuations();
        let world_updates = inner.world_updates.clone();

        let region_loc = RegionLocation::try_from_chunk_with_params(slab.chunk, &params)?;
        let region = inner.get_or_create_region(region_loc).await.unwrap(); // region loc checked above
        let chunk_desc = region.chunk(slab.chunk).description();

        // generate base slab terrain from chunk description
        trace!("generating slab terrain"; slab);
        let mut terrain = SlabGrid::default();
        chunk_desc.apply_to_slab(slab.slab, &mut terrain);

        // apply features to slab and collect subfeatures
        let slab_bounds = slab_bounds(slab);
        let (subfeatures_tx, mut subfeatures_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut ctx = ApplyFeatureContext {
            slab,
            chunk_desc,
            params: params.clone(),
            slab_bounds: &slab_bounds,
            subfeatures_tx,
        };

        // spawn a task to apply subfeatures to the terrain as they're produced
        let mut slab_continuations_for_task = slab_continuations.clone();

        let world_updates_task = world_updates.clone();
        let task = tokio::spawn(async move {
            let mut entities = vec![];
            while let Some(subfeature) = subfeatures_rx.recv().await {
                let entity = subfeature
                    .apply(
                        slab,
                        &mut terrain,
                        Some(&mut slab_continuations_for_task),
                        &params,
                        &world_updates_task,
                    )
                    .await;

                if let Some(e) = entity {
                    entities.push(e.0);
                }
            }

            (terrain, entities)
        });

        // apply relevant features
        for feature in region.features_for_slab(slab, &slab_bounds) {
            feature.apply_to_slab(&mut ctx).await;
        }

        // generate subfeatures not associated with any particular feature
        generate_loose_subfeatures(&mut ctx).await;

        // mark slab as completed
        let old_continuations = slab_continuations
            .lock()
            .await
            .insert(slab, SlabContinuation::Loaded);

        // ensure subfeature tx is dropped
        let params = ctx.params.clone();
        drop(ctx);

        // release lock on regions
        drop(region);
        drop(inner);

        // wait for all subfeatures to be rasterized
        let (mut terrain, entities) = task.await.expect("future panicked");

        // add any extra leaked subfeatures to this slab
        if let Some(SlabContinuation::Unloaded(extra)) = old_continuations {
            debug!("applying {count} leaked subfeatures to slab", count = extra.len(); slab);
            for subfeature in extra.into_iter() {
                // ignore entity description from other slabs, it's already handled by the owning
                // slab
                let _entity = subfeature
                    .apply(slab, &mut terrain, None, &params, &world_updates)
                    .await;
            }
        }

        Some(GeneratedPlanetSlab { terrain, entities })
    }

    pub async fn find_ground_level(&self, block: WorldPosition) -> Option<GlobalSliceIndex> {
        let inner = self.0.read().await;

        let chunk_loc = ChunkLocation::from(block);
        let region_loc = RegionLocation::try_from_chunk_with_params(chunk_loc, &inner.params)?;
        let region = inner.get_or_create_region(region_loc).await.unwrap(); // region loc checked above

        let chunk_desc = region.chunk(chunk_loc).description();
        let block_pos = BlockPosition::from(block);
        Some(chunk_desc.ground_level(block_pos.into()))
    }

    /// Instantiate regions and initialize chunks. Ignores those out of range
    // TODO wrap chunks rather than ignoring those out of range
    pub async fn prepare_for_chunks(&self, (min, max): (ChunkLocation, ChunkLocation)) {
        let regions = (min.0..=max.0)
            .cartesian_product(min.1..=max.1)
            .filter_map(|(cx, cy)| RegionLocation::try_from_chunk(ChunkLocation(cx, cy))) // TODO
            .dedup();

        for region in regions {
            self.realize_region(region).await;
        }
    }

    #[cfg(feature = "bin")]
    pub async fn inner(&self) -> impl std::ops::Deref<Target = PlanetInner> + '_ {
        self.0.read().await
    }

    pub async fn query_block(&self, block: WorldPosition) -> Option<BlockQueryResult> {
        let inner = self.0.read().await;
        let sampler = inner.continents.biome_sampler();
        let pos = PlanetPoint::from_block(block)?;
        let (coastline_proximity, base_elevation, moisture, temperature) =
            sampler.sample(pos, &inner.continents);
        let biomes =
            sampler.choose_biomes(coastline_proximity, base_elevation, temperature, moisture);

        let region = {
            let chunk = ChunkLocation::from(block);
            let region = RegionLocation::try_from_chunk(chunk);
            let region = match region {
                Some(loc) => inner.regions.get_existing(loc).await.map(|r| (loc, r)),
                None => None,
            };
            region.map(|(loc, region)| {
                let slab = SlabLocation::new(SlabIndex::from(block.slice()), chunk);
                let slab_bounds = slab_bounds(slab);
                let features = region
                    .features_for_slab(slab, &slab_bounds)
                    .filter_map(move |feature| {
                        if feature.applies_to_block(block) {
                            Some(format!("{}", feature.display()))
                        } else {
                            None
                        }
                    })
                    .collect();
                (loc, features)
            })
        };

        Some(BlockQueryResult {
            biome_choices: biomes,
            coastal_proximity: coastline_proximity,
            base_elevation,
            moisture,
            temperature,
            region,
        })
    }

    /// Sorts and dedups the given chunk stream into regions, gets all regional features in the
    /// given z range, calls given closure on each point of the boundary.
    ///
    /// Nop if feature mutex is not immediately available, i.e. does not block
    pub async fn feature_boundaries_in_range(
        &self,
        chunks: impl Iterator<Item = ChunkLocation>,
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        mut per_point: impl FnMut(usize, WorldPosition),
    ) {
        let inner = self.0.read().await;
        for region in chunks
            .filter_map(|c| RegionLocation::try_from_chunk_with_params(c, &inner.params))
            .sorted_unstable() // allocation, gross
            .dedup()
        {
            if let Some(region) = inner.regions.get_existing(region).await {
                for feature in region.all_features() {
                    let unique = feature.unique_id();
                    feature.bounding_points(z_range, |point| {
                        per_point(unique, point.into_block(z_range.1))
                    });
                }
            }
        }
    }

    pub async fn steal_world_updates(
        &self,
        with_updates: impl FnOnce(std::vec::Drain<(WorldPosition, GeneratedBlock)>),
    ) {
        let inner = self.0.write().await;
        let mut updates = inner.world_updates.lock().await;
        with_updates(updates.drain(..));
    }
}

impl PlanetInner {
    async fn get_or_create_region(&self, region: RegionLocation) -> Option<LoadedRegionRef<'_>> {
        self.regions.get_or_create(region, &self.continents).await
    }
}

/// Expensive, result should be cached
///
/// Panics if slab location is invalid
pub(crate) fn slab_bounds(slab: SlabLocation) -> Rect<f64> {
    let min = SlabPosition::new_unchecked(0, 0, SliceIndex::bottom()).to_world_position(slab);
    let min_point = PlanetPoint::from_block(min).unwrap(); // slab location assumed to be fine

    let min_coord = Coordinate::from(min_point.get_array());
    let max_coord = {
        let offset = PlanetPoint::PER_BLOCK * CHUNK_SIZE.as_f64();
        min_coord + Coordinate::from([offset, offset])
    };

    Rect::new(min_coord, max_coord)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegionLocation;
    use geo::coords_iter::CoordsIter;

    #[test]
    fn slab_bounds_in_region() {
        let region = RegionLocation::new(5, 6);
        let slab = region.chunk_bounds().0.get_slab(8);

        let bounds = slab_bounds(slab);
        for coord in bounds.coords_iter() {
            let (x, y) = coord.x_y();
            let (rx, ry) = region.xy();

            assert_eq!(rx, x.floor() as u32);
            assert_eq!(ry, y.floor() as u32);
        }

        // square and 1 chunk in size
        assert_eq!(
            bounds.height(),
            PlanetPoint::PER_BLOCK * CHUNK_SIZE.as_f64()
        );
        assert_eq!(bounds.height(), bounds.width());
    }

    #[test]
    fn slab_bounds_vary() {
        let region = RegionLocation::new(5, 6);
        let chunk = region.chunk_bounds().0;

        // differ horizontally
        let a = {
            let chunk: ChunkLocation = chunk + (2, 2);
            slab_bounds(chunk.get_slab(4))
        };
        let b = {
            let chunk: ChunkLocation = chunk + (3, 2);
            slab_bounds(chunk.get_slab(4))
        };

        assert_ne!(a, b);

        // differ vertically
        let a = {
            let chunk: ChunkLocation = chunk + (2, 2);
            slab_bounds(chunk.get_slab(4))
        };
        let b = {
            let chunk: ChunkLocation = chunk + (2, 2);
            slab_bounds(chunk.get_slab(9))
        };

        assert_eq!(a, b);
    }
}
