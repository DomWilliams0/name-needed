use std::collections::{HashMap, HashSet};
use std::mem::MaybeUninit;
use std::sync::Arc;

use geo::{Point, Rect};
use tokio::sync::{Mutex, RwLock};

pub use ::unit::world::{
    ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SliceBlock, SliceIndex,
    CHUNK_SIZE, SLAB_SIZE,
};
use common::*;
use grid::{grid_declare, GridImpl};
use unit::world::{BlockPosition, SlabLocation, SlabPosition};

use crate::biome::BiomeType;
use crate::continent::ContinentMap;
use crate::rasterize::BlockType;
use crate::region::feature::{FeatureZRange, RegionalFeatureBoundary, SharedRegionalFeature};
use crate::region::features::ForestFeature;
use crate::region::row_scanning::RegionNeighbour;
use crate::region::unit::PlanetPoint;
use crate::region::RegionalFeature;
use crate::{
    map_range, region::unit::RegionLocation, SlabGrid, SlabPositionAsCoord, SliceBlockAsCoord,
};

use crate::params::PlanetParamsRef;

use crate::region::subfeature::SlabContinuation;
use geo::prelude::HasDimensions;

pub struct Regions<const SIZE: usize, const SIZE_2: usize> {
    params: PlanetParamsRef,
    regions: Vec<(RegionLocation<SIZE>, Region<SIZE, SIZE_2>)>,

    loaded_regions: LoadedRegions<SIZE>,
    region_continuations: RegionContinuations<SIZE>,
    slab_continuations: SlabContinuations,
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

type LoadedRegions<const SIZE: usize> = Arc<RwLock<HashSet<RegionLocation<SIZE>>>>;
type RegionContinuations<const SIZE: usize> =
    Arc<Mutex<HashMap<RegionLocation<SIZE>, RegionContinuation<SIZE>>>>;
pub(super) type SlabContinuations = Arc<Mutex<HashMap<SlabLocation, SlabContinuation>>>;

/// Info about features/generation from neighbouring regions that is to be carried over the
/// boundary
#[derive(Default, Debug)]
struct RegionContinuation<const SIZE: usize> {
    /// (direction of neighbour from this region, feature)
    features: Vec<(RegionNeighbour, SharedRegionalFeature)>,
}

struct RegionalFeatureReplacement<const SIZE: usize> {
    region: RegionLocation<SIZE>,
    current: SharedRegionalFeature,
    new: SharedRegionalFeature,
}
type RegionalFeatureReplacements<const SIZE: usize> =
    SmallVec<[RegionalFeatureReplacement<SIZE>; 4]>;

pub struct RegionChunksBlockRows<'a, const SIZE: usize>(&'a [RegionChunk<SIZE>]);

// TODO rename me
#[derive(Debug, Clone, Copy)]
pub struct BlockHeight {
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
    pub fn new(params: PlanetParamsRef) -> Self {
        Regions {
            params,
            regions: Vec::with_capacity(64),
            region_continuations: Arc::new(Mutex::new(HashMap::with_capacity(64))),
            loaded_regions: Arc::new(RwLock::new(HashSet::with_capacity(128))),
            slab_continuations: Arc::new(Mutex::new(HashMap::with_capacity(64))),
        }
    }

    pub async fn get_or_create(
        &mut self,
        location: RegionLocation<SIZE>,
        continents: &ContinentMap,
    ) -> Option<&Region<SIZE, SIZE_2>> {
        let ret = Some(match self.region_index(location)? {
            Ok(idx) => &self.regions[idx].1,
            Err(idx) => {
                debug!("creating new region"; "region" => ?location);

                let (region, feature_updates) = Region::create(
                    location,
                    continents,
                    self.region_continuations.clone(), // wrapper around Arc
                    self.loaded_regions.clone(),
                    self.params.clone(),
                )
                .await;

                // apply feature replacements
                // TODO is there a race condition where a region that's supposed to replace a feature
                //  swaps it with another before it can be replaced here?
                for RegionalFeatureReplacement {
                    region,
                    current,
                    new,
                } in feature_updates
                {
                    debug!("applying feature replacement"; "region" => ?region, "current" => ?current.ptr_debug(), "new" => ?new.ptr_debug());
                    let idx = match self.region_index(region) {
                        Some(Ok(idx)) => idx,
                        _ => unreachable!("invalid neighbour {:?} returned", region),
                    };

                    let (_, region) = &mut self.regions[idx];

                    if let Some(feature) = region
                        .features
                        .iter_mut()
                        .find(|f| Arc::ptr_eq(&current, *f))
                    {
                        // swapadoodledoo
                        *feature = new;
                    }
                }

                self.regions.insert(idx, (location, region));
                self.loaded_regions.write().await.insert(location);

                &self.regions[idx].1
            }
        });

        ret
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

    pub fn slab_continuations(&self) -> SlabContinuations {
        Arc::clone(&self.slab_continuations)
    }
}

impl<const SIZE: usize, const SIZE_2: usize> Region<SIZE, SIZE_2> {
    async fn create<'c>(
        loc: RegionLocation<SIZE>,
        continents: &ContinentMap,
        continuations: RegionContinuations<SIZE>,
        loaded_regions: LoadedRegions<SIZE>,
        params: PlanetParamsRef,
    ) -> (Self, RegionalFeatureReplacements<SIZE>) {
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
        // TODO region chunk load order affects feature generations -
        //  if multiple unconnected regions covering the same feature (e.g. big forest) are loaded,
        //  they'll spawn distinct forest instances and spread them around until they meet, then
        //  1 will take over a few regions but not all. if features will randomly generate some
        //  characteristics e.g. tree type, the merging will combine them with an obvious mismatch
        //  along region boundaries!
        //
        //  maybe a loading region could somehow query for nearby forests in a radius to seed itself
        //  with, or only discover regional features in a flood-fill pattern, i.e. keep yielding until
        //  a region neighbour is loaded, or trigger neighbour loading up to the other one? this
        //  would disallow/ruin random slab loading..
        let updates = region
            .discover_regional_features(loc, continuations, loaded_regions, &params)
            .await;

        (region, updates)
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
        loaded_regions: LoadedRegions<SIZE>,
        params: &PlanetParamsRef,
    ) -> RegionalFeatureReplacements<SIZE> {
        // expand each row outwards a tad for slightly relaxed boundary
        let expansion = params.region_feature_expansion as f64 * PlanetPoint::<SIZE>::PER_BLOCK;

        let mut points = Vec::new();
        let mut feature_range = FeatureZRange::null();
        let mut y_range = (f64::MAX, f64::MIN);
        let mut overflows =
            super::row_scanning::scan(self.block_rows(), BiomeType::Forest, |forest_row| {
                feature_range = feature_range.max_of(forest_row.z_range);

                points.extend(
                    forest_row
                        .into_points_with_expansion(region, expansion)
                        .into_iter()
                        .map(|point| Point::from(point.get_array()))
                        .inspect(|point| {
                            let (min, max) = y_range;
                            y_range = (min.min(point.y()), max.max(point.y()));
                        }),
                );
            });

        let mut feature_updates = SmallVec::new();

        if points.is_empty() {
            // no feature, yippee
            return feature_updates;
        }

        debug_assert_ne!(feature_range, FeatureZRange::null());
        debug_assert_ne!(y_range, (f64::MAX, f64::MIN));

        let (mut bounding, n) = RegionalFeatureBoundary::new::<SIZE>(points, y_range, params);
        trace!("regional feature discovery"; "region" => ?region, "points" => n, "overflows" => ?overflows);

        // must only be called once, result is cached in this_feature
        let mut this_feature: Option<SharedRegionalFeature> = None;
        fn create_new_feature(
            bounding: &mut RegionalFeatureBoundary,
            feature_range: FeatureZRange,
            params: &PlanetParamsRef,
        ) -> SharedRegionalFeature {
            let bounding = {
                let stolen = std::mem::take(bounding);
                assert!(!stolen.is_empty()); // is only called once
                stolen
            };

            RegionalFeature::new(bounding, feature_range, ForestFeature::new(params))
        }

        // take continuations mutex now and don't release until self and all neighbours are updated,
        // to avoid a TOCTOU where a region pops its empty continuation here, and is allocated a new
        // populated one just after, which it never checks
        let mut continuations_guard = continuations.lock().await;

        // pop continuations for this region
        let mut continuation = continuations_guard.remove(&region).unwrap_or_default();
        trace!("continuations"; "region" => ?region, "continuation" => ?continuation);

        // sort neighbours with confirmed continuations to the front, so we hit them first and copy
        // a reference to their preexisting feature, instead of creating a new feature here
        // then having to merge
        overflows.sort_unstable_by_key(|o| !continuation.contains(o));

        for overflow in overflows.into_iter() {
            let neighbour =
                match region.try_add_offset_with_params(overflow.offset::<SIZE>(), &params) {
                    Some(n) => n,
                    None => continue, // out of bounds, nvm
                };

            // TODO will need to filter on feature type when there are multiple
            if let Some(other_feature) = continuation.pop(overflow) {
                // neighbour already has a feature
                match this_feature.as_ref() {
                    None => {
                        // use theirs as we don't have one yet
                        trace!("using neighbour's feature instance"; "region" => ?region,
                            "neighbour" => ?neighbour, "feature" => ?other_feature.ptr_debug());

                        let bounding = std::mem::take(&mut bounding);
                        debug_assert!(!bounding.is_empty()); // consumed only once
                        other_feature.merge_with_bounds(bounding, feature_range);
                        this_feature = Some(other_feature);
                    }
                    Some(f) if !SharedRegionalFeature::ptr_eq(f, &other_feature) => {
                        // replace theirs with ours (after return)
                        trace!("replacing neighbour's feature instance with ours";
                            "region" => ?region, "neighbour" => ?neighbour,
                            "theirs" => ?other_feature.ptr_debug(), "ours" => ?f.ptr_debug());

                        feature_updates.push(RegionalFeatureReplacement {
                            region: neighbour,
                            current: other_feature.clone(),
                            new: f.clone(),
                        });

                        f.merge_with_other(other_feature)
                            .await
                            .expect("regional feature type mismatch");
                    }
                    Some(_) => {
                        trace!("neighbour already has the same feature as us";
                            "region" => ?region, "neighbour" => ?neighbour,
                            "feature" => ?other_feature.ptr_debug());
                    }
                };
            } else {
                // neighbour does not have a feature continuation, use own feature
                let feature = match this_feature {
                    Some(ref f) => {
                        // already created one, reuse it
                        trace!("reusing own feature"; "region" => ?region,
                            "neighbour" => ?neighbour, "feature" => ?f.ptr_debug());
                        f.clone()
                    }
                    None => {
                        let feature = create_new_feature(&mut bounding, feature_range, params);
                        trace!("created new feature"; "region" => ?region,
                            "neighbour" => ?neighbour, "feature" => ?feature.ptr_debug());
                        this_feature = Some(feature.clone());
                        feature
                    }
                };

                // add feature to neighbour's continuations if it hasn't already been loaded. if it's
                // already loaded and didn't register a continuation then this is a false positive
                // where e.g. the feature ends exactly at the region edge
                if !loaded_regions.read().await.contains(&neighbour) {
                    trace!("adding feature to unloaded neighbour's continuations"; "region" => ?region,
                            "neighbour" => ?neighbour, "feature" => ?feature.ptr_debug());
                    let neighbour_continuations = continuations_guard
                        .entry(neighbour)
                        .or_insert_with(RegionContinuation::default);

                    neighbour_continuations
                        .features
                        .push((overflow.opposite(), feature))
                } else {
                    trace!("neighbour is already loaded, skipping continuation"; "region" => ?region,
                            "neighbour" => ?neighbour, "feature" => ?feature.ptr_debug());
                }
            }
        }

        drop(continuations_guard);

        // add the new feature to this region
        let feature = this_feature
            .take()
            .unwrap_or_else(|| create_new_feature(&mut bounding, feature_range, params));
        self.features.push(feature);

        feature_updates
    }

    #[cfg(any(test, feature = "benchmarking"))]
    #[inline]
    pub async fn create_for_benchmark(
        region: RegionLocation<SIZE>,
        continents: &ContinentMap,
        params: PlanetParamsRef,
    ) -> Self {
        // TODO null params for benchmark
        Self::create(region, continents, todo!(), todo!(), params)
            .await
            .0
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

    pub fn features_for_slab<'a>(
        &'a self,
        slab: SlabLocation,
        slab_bounds: &'a Rect<f64>,
    ) -> impl Iterator<Item = &SharedRegionalFeature> + 'a {
        self.features
            .iter()
            .filter(move |feature| feature.applies_to(slab, slab_bounds))
    }

    pub fn all_features(&self) -> impl Iterator<Item = &SharedRegionalFeature> + '_ {
        self.features.iter()
    }

    /// Iterator that yields rows of blocks across the entire region. Fiddly because of the memory
    /// layout of each region chunk
    pub fn block_rows(&self) -> RegionChunksBlockRows<'_, SIZE> {
        RegionChunksBlockRows(&self.chunks)
    }
}

impl<'a, const SIZE: usize> RegionChunksBlockRows<'a, SIZE> {
    pub fn blocks(self) -> impl Iterator<Item = &'a BlockHeight> + 'a {
        (0..SIZE)
            .map(move |col| {
                let row_offset = col * SIZE;
                &self.0[row_offset..row_offset + SIZE]
            })
            .flat_map(move |row_of_chunks| {
                (0..CHUNK_SIZE.as_usize())
                    .cartesian_product(0..SIZE)
                    .cartesian_product(0..CHUNK_SIZE.as_usize())
                    .map(move |((by, cx), bx)| {
                        debug_assert!(cx < row_of_chunks.len());
                        // safety: cx is limited to 0..SIZE, same as slice len
                        let chunk = unsafe { row_of_chunks.get_unchecked(cx) };

                        let i = (by * CHUNK_SIZE.as_usize()) + bx;
                        chunk.desc.ground_height.index(i).unwrap() // index is definitely valid
                    })
            })
    }

    #[cfg(test)]
    pub fn with_chunks(chunks: &'a [RegionChunk<SIZE>]) -> Self {
        Self(chunks)
    }
}

impl<const SIZE: usize> RegionContinuation<SIZE> {
    fn pop(&mut self, neighbour: RegionNeighbour) -> Option<SharedRegionalFeature> {
        let idx = self.features.iter().position(|(n, _)| *n == neighbour)?;
        Some(self.features.swap_remove(idx).1)
    }

    fn contains(&self, neighbour: &RegionNeighbour) -> bool {
        self.features.iter().any(|(n, _)| n == neighbour)
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

            let block = height_map.index_mut(i).unwrap(); // index is certainly valid
            *block = BlockHeight {
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

            for (i, (y, x)) in (0..CHUNK_SIZE.as_u8())
                .cartesian_product(0..CHUNK_SIZE.as_u8())
                .enumerate()
            {
                let pos = SlabPosition::new_unchecked(x, y, 0.into());
                let BlockHeight { ground, biome } =
                    *self.ground_height.get_unchecked(SlabPositionAsCoord(pos));

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
        self.block(block).ground
    }

    pub fn block(&self, block: SliceBlock) -> &BlockHeight {
        self.ground_height.get_unchecked(SliceBlockAsCoord(block))
    }

    /// Iterator over the block descriptions in this chunk. Note the order is per row, i.e. for
    /// a chunk size of 4:
    ///
    /// ```none
    /// 12  13  14  15
    /// 8   9   10  11
    /// 4   5   6   7
    /// 0   1   2   3
    /// ```
    pub(crate) fn blocks(&self) -> &[BlockHeight] {
        &self.ground_height.array()
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
    use common::thread_rng;
    use unit::dim::SmallUnsignedConstant;
    use unit::world::ChunkLocation;

    use crate::continent::ContinentMap;
    use crate::params::PlanetParamsRef;
    use crate::region::region::{Region, Regions};
    use crate::region::unit::RegionLocation;
    use crate::PlanetParams;

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
            let mut params_mut = PlanetParamsRef::get_mut(&mut params).unwrap();
            params_mut.planet_size = 32;
            params_mut.max_continents = 1;
            params
        };
        let mut regions = SmolRegions::new(params.clone());
        let continents = ContinentMap::new_with_rng(params.clone(), &mut thread_rng());

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
