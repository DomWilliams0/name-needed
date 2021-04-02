use std::mem::MaybeUninit;

pub use ::unit::world::{
    ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SliceBlock, SliceIndex,
    CHUNK_SIZE, SLAB_SIZE,
};
use common::*;
use grid::{grid_declare, GridImpl};

use crate::biome::BiomeType;
use crate::continent::ContinentMap;
use crate::rasterize::BlockType;
use crate::region::feature::{FeatureZRange, ForestFeature, SharedRegionalFeature};
use crate::region::unit::PlanetPoint;
use crate::region::RegionalFeature;
use crate::{map_range, region::unit::RegionLocation, PlanetParams, SlabGrid};
use geo::concave_hull::ConcaveHull;
use geo::{MultiPoint, Point};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use unit::world::{BlockPosition, SlabLocation};

pub struct Regions<const SIZE: usize, const SIZE_2: usize> {
    params: PlanetParams,
    // TODO helper struct for a sorted Vec as a key value lookup, instead of repeating boilerplate
    regions: Vec<(RegionLocation<SIZE>, Region<SIZE, SIZE_2>)>,

    continuations: RegionContinuations<SIZE>,
}

/// Each pixel in the continent map is a region. Each region is a 2d grid of chunks.
///
/// Large scale features are generated globally (forest placement, rivers, ore distributions, cave
/// placement, etc) but only stored until a slab is requested. When a range of slabs is
/// requested, initialize all chunks in the region and apply features to slabs in the vertical range.
///
/// Chunk initialization:
///     * Calculate description from block distribution based on position. This is only a
///       description and is not yet rasterized into blocks. e.g.
///        * all air if above ground
///        * surface blocks (grass, stone etc) if at ground level based on heightmap
///        * solid stone underground
///
/// For every large feature that overlaps with this region (in all
/// axes including z, so all underground caves aren't calculated now if only the surface is being
/// generated):
///     * Generate subfeatures if relevant and not already done (tree placement in forest bounds,
///       river curve rasterization into blocks, etc)
///     * Attempt to place all blocks in each subfeature in this region and slab range only
///         * The first time a slab is touched, use chunk description to rasterize initial blocks
// TODO when const generics can be used in evaluations, remove stupid SIZE_2 type param (SIZE * SIZE)
pub struct Region<const SIZE: usize, const SIZE_2: usize> {
    chunks: [RegionChunk<SIZE>; SIZE_2],
    features: Vec<SharedRegionalFeature>,
}

pub struct RegionChunk<const SIZE: usize> {
    desc: ChunkDescription,
}

pub struct ChunkDescription {
    ground_height: ChunkHeightMap,
}

/// Info about features/generation from neighbouring regions that is to be carried over the
/// boundary
struct RegionContinuation {}

#[derive(Clone)]
struct RegionContinuations<const SIZE: usize>(
    Arc<Mutex<HashMap<RegionLocation<SIZE>, RegionContinuation>>>,
);

// TODO rename me
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockHeight {
    ground: GlobalSliceIndex,
    biome: BiomeType,
}

grid_declare!(pub(crate) struct ChunkHeightMap<ChunkHeightMapImpl, BlockHeight>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    1
);

impl Default for BlockHeight {
    fn default() -> Self {
        // not important, will be overwritten by real values
        Self {
            ground: GlobalSliceIndex::bottom(),
            biome: BiomeType::Ocean,
        }
    }
}

impl<const SIZE: usize, const SIZE_2: usize> Regions<SIZE, SIZE_2> {
    pub fn new(params: &PlanetParams) -> Self {
        Regions {
            params: params.clone(),
            regions: Vec::with_capacity(64),
            continuations: RegionContinuations::default(),
        }
    }

    pub async fn get_or_create(
        &mut self,
        location: RegionLocation<SIZE>,
        continents: &ContinentMap,
    ) -> Option<&Region<SIZE, SIZE_2>> {
        Some(match self.region_index(location)? {
            Ok(idx) => &self.regions[idx].1,
            Err(idx) => {
                debug!("creating new region"; "region" => ?location);

                let region = Region::create(
                    location,
                    continents,
                    self.continuations.clone(), // wrapper around Arc
                    &self.params,
                )
                .await;
                self.regions.insert(idx, (location, region));
                &self.regions[idx].1
            }
        })
    }

    pub fn get_existing(&self, region: RegionLocation<SIZE>) -> Option<&Region<SIZE, SIZE_2>> {
        self.region_index(region)
            .and_then(|idx| idx.ok())
            .map(|idx| &self.regions[idx].1)
    }

    /// None if out of range of the planet, otherwise Ok(idx) if present or Err(idx) if in range but
    /// not present
    fn region_index(&self, region: RegionLocation<SIZE>) -> Option<Result<usize, usize>> {
        self.params
            .is_region_in_range(region)
            .as_some_from(|| self.regions.binary_search_by_key(&region, |(pos, _)| *pos))
    }
}

impl<const SIZE: usize, const SIZE_2: usize> Region<SIZE, SIZE_2> {
    async fn create<'c>(
        loc: RegionLocation<SIZE>,
        continents: &ContinentMap,
        continuations: RegionContinuations<SIZE>,
        _params: &PlanetParams,
    ) -> Self {
        debug_assert_eq!(SIZE * SIZE, SIZE_2); // gross but temporary as long as we need SIZE_2

        // using a log_scope here causes a nested panic, possibly due to dropping the scope multiple
        // times?
        debug!("creating region"; "region" => ?loc);

        // initialize terrain description for chunks, and sample biome at each block
        let chunks = Self::init_region_chunks(loc, continents).await;

        let mut region = Region {
            chunks,
            features: Vec::with_capacity(16),
        };

        // regional feature discovery
        region.discover_regional_features(loc, continuations).await;

        region
    }

    async fn init_region_chunks(
        region: RegionLocation<SIZE>,
        continents: &ContinentMap,
    ) -> [RegionChunk<SIZE>; SIZE_2] {
        // initialize chunk descriptions
        let mut chunks: [MaybeUninit<RegionChunk<SIZE>>; SIZE_2] =
            unsafe { MaybeUninit::uninit().assume_init() };

        let continents: &'static ContinentMap = unsafe { std::mem::transmute(continents) };

        let handle = tokio::runtime::Handle::current();
        let results = futures::future::join_all((0..SIZE_2).map(|idx| {
            // cant pass a ptr across threads but you can an integer :^)
            // the array is stack allocated and we dont leave this function while this closure is
            // alive so this pointer is safe to use.
            let this_chunk = chunks[idx].as_mut_ptr() as usize;
            handle.spawn(async move {
                let chunk = RegionChunk::new(idx, region, continents);

                // safety: each task has a single index in the chunk array
                unsafe {
                    let this_chunk = this_chunk as *mut RegionChunk<SIZE>;
                    this_chunk.write(chunk);
                }
            })
        }))
        .await;

        for result in results {
            if let Err(err) = result {
                panic!("panic occurred in future: {}", err);
            }
        }

        // safety: all chunks have been initialized and any panics have been propagated
        let chunks: [RegionChunk<SIZE>; SIZE_2] = unsafe {
            let ptr = &mut chunks as *mut _ as *mut [RegionChunk<SIZE>; SIZE_2];
            let res = ptr.read();
            core::mem::forget(chunks);
            res
        };

        chunks
    }

    async fn discover_regional_features(
        &mut self,
        region: RegionLocation<SIZE>,
        continuations: RegionContinuations<SIZE>,
    ) {
        let mut points = Vec::new();
        let mut feature_range = FeatureZRange::null();
        let overflows = super::row_scanning::scan(&self.chunks, BiomeType::Forest, |forest_row| {
            feature_range = feature_range.max_of(forest_row.z_range);
            points.extend(forest_row.into_points(region).map(|(x, y)| {
                // centre of each block
                Point::new(x as f64 + 0.5, y as f64 + 0.5)
            }));
        });

        if points.is_empty() {
            // no feature, yippee
            return;
        }

        debug_assert_ne!(feature_range, FeatureZRange::null());

        let bounding = {
            let points = MultiPoint(points);
            let polygon = points.concave_hull(2.0);
            // TODO expand polygon out to ensure it covers the entire biome area?
            // polygon.simplify(&1.0);
            polygon
        };

        // pop continuations for this region
        let continuation = continuations.pop(region).await;

        // TODO check continuations to see if this is the extension of an existing feature
        // TODO pass onto the overflow regions for continuation

        let feature = RegionalFeature::new(bounding, feature_range, ForestFeature::default());
        self.features.push(feature);
    }
    #[cfg(any(test, feature = "benchmarking"))]
    #[inline]
    pub async fn create_for_benchmark(
        region: RegionLocation<SIZE>,
        continents: &ContinentMap,
        params: &PlanetParams,
    ) -> Self {
        // TODO null continuations for benchmark
        Self::create(region, continents, todo!(), params).await
    }

    pub(crate) fn chunk_index(chunk: ChunkLocation) -> usize {
        let ChunkLocation(x, y) = chunk;
        let x = x.rem_euclid(SIZE as i32);
        let y = y.rem_euclid(SIZE as i32);

        (x + (y * SIZE as i32)) as usize
    }

    pub fn chunk(&self, chunk: ChunkLocation) -> &RegionChunk<SIZE> {
        let idx = Self::chunk_index(chunk);
        debug_assert!(idx < self.chunks.len(), "bad idx {}", idx);
        &self.chunks[idx]
    }

    pub fn features_for_slab(
        &self,
        slab: SlabLocation,
    ) -> impl Iterator<Item = &SharedRegionalFeature> + '_ {
        self.features
            .iter()
            .filter(move |feature| feature.applies_to(slab))
    }
}

impl<const SIZE: usize> Default for RegionContinuations<SIZE> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(HashMap::with_capacity(32))))
    }
}

impl<const SIZE: usize> RegionContinuations<SIZE> {
    async fn pop(&self, region: RegionLocation<SIZE>) -> Option<RegionContinuation> {
        let mut guard = self.0.lock().await;
        guard.remove(&region)
    }
}

impl<const SIZE: usize> RegionChunk<SIZE> {
    fn new(chunk_idx: usize, region: RegionLocation<SIZE>, continents: &ContinentMap) -> Self {
        let precalc = PlanetPoint::precalculate(region, chunk_idx);
        let sampler = continents.biome_sampler();

        // get height for each surface block in chunk
        let mut height_map = ChunkHeightMap::default();
        let (mut min_height, mut max_height) = (i32::MAX, i32::MIN);
        for (i, (by, bx)) in (0..CHUNK_SIZE.as_u8())
            .cartesian_product(0..CHUNK_SIZE.as_u8())
            .enumerate()
        {
            let point =
                PlanetPoint::with_precalculated(&precalc, BlockPosition::new(bx, by, 0.into()));

            let (coastal, base_elevation, moisture, temperature) =
                sampler.sample(point, continents);

            let biome_choices =
                sampler.choose_biomes(coastal, base_elevation, temperature, moisture);
            let biome = biome_choices.primary();

            // get block height from elevation, weighted by biome(s)
            let height_range = {
                biome_choices
                    .choices()
                    .map(|(biome, weight)| {
                        let (min, max) = biome.elevation_range();
                        let (min, max) = (min as f32, max as f32);
                        (min * weight.value(), max * weight.value())
                    })
                    .fold((0.0, 0.0), |acc, range| (acc.0 + range.0, acc.1 + range.1))
            };
            let ground =
                GlobalSliceIndex::new(
                    map_range((0.0, 1.0), height_range, base_elevation as f32) as i32
                );

            height_map[i] = BlockHeight {
                ground,
                biome: biome.ty(),
            };
            min_height = min_height.min(ground.slice());
            max_height = max_height.max(ground.slice());
        }

        // TODO depends on many local parameters e.g. biome, humidity

        trace!("generated region chunk"; "chunk" => ?precalc.chunk(), "region" => ?precalc.region());

        RegionChunk {
            desc: ChunkDescription {
                ground_height: height_map,
            },
        }
    }

    pub fn description(&self) -> &ChunkDescription {
        &self.desc
    }

    #[cfg(test)]
    pub fn empty() -> Self {
        RegionChunk {
            desc: ChunkDescription {
                ground_height: Default::default(),
            },
        }
    }
    #[cfg(test)]
    pub(crate) fn biomes_mut(&mut self) -> &mut ChunkHeightMap {
        &mut self.desc.ground_height
    }
}

impl ChunkDescription {
    pub fn apply_to_slab(&self, slab_idx: SlabIndex, slab: &mut SlabGrid) {
        let from_slice = slab_idx.as_i32() * SLAB_SIZE.as_i32();
        let to_slice = from_slice + SLAB_SIZE.as_i32();

        // TODO could do this multiple slices at a time
        for (z_global, z_local) in (from_slice..to_slice)
            .map(GlobalSliceIndex::new)
            .zip(LocalSliceIndex::range())
        {
            let slice = {
                let (from, to) = slab.slice_range(z_local.slice_unsigned());
                &mut slab.array_mut()[from..to]
            };

            for (i, (y, x)) in (0..CHUNK_SIZE.as_i32())
                .cartesian_product(0..CHUNK_SIZE.as_i32())
                .enumerate()
            {
                let BlockHeight { ground, biome } = self.ground_height[&[x, y, 0]];

                // TODO calculate these better, and store them in data
                use BlockType::*;
                let (surface_block, shallow_under_block, deep_under_block, shallow_depth) =
                    match biome {
                        BiomeType::Ocean | BiomeType::IcyOcean | BiomeType::CoastOcean => {
                            (Dirt, Sand, Stone, 1)
                        }
                        BiomeType::Beach => (Sand, Dirt, Stone, 4),
                        BiomeType::Plains => (LightGrass, Dirt, Stone, 3),
                        BiomeType::Forest | BiomeType::Tundra => (Grass, Dirt, Stone, 3),
                        BiomeType::Desert => (Sand, Sand, Stone, 6),
                    };

                let bt = match (ground - z_global).slice() {
                    0 => surface_block,
                    d if d.is_negative() => BlockType::Air,
                    d if d < shallow_depth => shallow_under_block,
                    _ => deep_under_block,
                };

                slice[i].ty = bt;
            }
        }
    }

    pub fn ground_level(&self, block: SliceBlock) -> GlobalSliceIndex {
        let SliceBlock(x, y) = block;
        self.ground_height[&[x as i32, y as i32, 0]].ground
    }

    pub(crate) fn blocks(&self) -> impl Iterator<Item = &BlockHeight> + '_ {
        self.ground_height.array().iter()
    }
}

impl BlockHeight {
    pub const fn biome(&self) -> BiomeType {
        self.biome
    }

    pub const fn ground(&self) -> GlobalSliceIndex {
        self.ground
    }

    #[cfg(test)]
    pub fn set_biome(&mut self, biome: BiomeType) {
        self.biome = biome;
    }
}

// slog_value_debug!(RegionLocation);

#[cfg(test)]
mod tests {
    use crate::continent::ContinentMap;
    use crate::region::region::{Region, Regions};
    use crate::region::unit::RegionLocation;
    use crate::PlanetParams;
    use common::thread_rng;
    use unit::dim::SmallUnsignedConstant;
    use unit::world::ChunkLocation;

    const SIZE: SmallUnsignedConstant = SmallUnsignedConstant::new(4);
    type SmolRegionLocation = RegionLocation<4>;
    type SmolRegion = Region<4, 16>;
    type SmolRegions = Regions<4, 16>;

    #[test]
    fn chunk_to_region() {
        // negative is always out of range
        assert_eq!(
            SmolRegionLocation::try_from_chunk(ChunkLocation(-2, 1)),
            None
        );

        assert_eq!(
            SmolRegionLocation::try_from_chunk(ChunkLocation(SIZE.as_i32() / 2, SIZE.as_i32())),
            Some(SmolRegionLocation::new(0, 1))
        );
    }

    #[test]
    fn chunk_index() {
        assert_eq!(
            SmolRegion::chunk_index(ChunkLocation(0, 2)),
            SIZE.as_usize() * 2
        );

        assert_eq!(SmolRegion::chunk_index(ChunkLocation(3, 0)), 3);

        assert_eq!(
            SmolRegion::chunk_index(ChunkLocation(3 + (SIZE.as_i32() * 3), 0)),
            3
        );

        let idx = SmolRegion::chunk_index(ChunkLocation(3, 2));
        assert_eq!(idx, 11);
        assert_eq!(SmolRegion::chunk_index(ChunkLocation(-1, -2)), idx);
    }

    #[tokio::test]
    async fn get_existing_region() {
        let params = {
            let mut params = PlanetParams::dummy();
            params.planet_size = 32;
            params.max_continents = 1;
            params
        };
        let mut regions = SmolRegions::new(&params);
        let continents = ContinentMap::new_with_rng(&params, &mut thread_rng());

        let loc = SmolRegionLocation::new(10, 20);
        let bad_loc = SmolRegionLocation::new(10, 200);

        assert!(regions.get_existing(loc).is_none());
        assert!(regions.get_existing(bad_loc).is_none());

        assert!(params.is_region_in_range(loc));
        assert!(!params.is_region_in_range(bad_loc));

        assert!(regions.get_or_create(loc, &continents).await.is_some());
        assert!(regions.get_or_create(bad_loc, &continents).await.is_none());

        assert!(regions.get_existing(loc).is_some());
        assert!(regions.get_existing(bad_loc).is_none());
    }
}
