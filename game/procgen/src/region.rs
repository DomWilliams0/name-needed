use crate::continent::Generator;
use crate::rasterize::BlockType;
use crate::{map_range, PlanetParams, SlabGrid};
use common::*;

use grid::{grid_declare, GridImpl};
use std::cmp::Ordering;

use std::mem::MaybeUninit;
use std::sync::Arc;

use unit::dim::SmallUnsignedConstant;
use unit::world::{
    BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SliceIndex,
    CHUNK_SIZE, SLAB_SIZE,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RegionLocation(pub i32, pub i32);

/// Each region is broken up into this many chunks per side, i.e. this^2 for total number of chunks
const CHUNKS_PER_REGION_SIDE: SmallUnsignedConstant = SmallUnsignedConstant::new(8);

const CHUNKS_PER_REGION: usize =
    CHUNKS_PER_REGION_SIDE.as_usize() * CHUNKS_PER_REGION_SIDE.as_usize();

pub struct Regions {
    params: PlanetParams,
    regions: Vec<(RegionLocation, Region)>,
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
pub struct Region {
    chunks: [RegionChunk; CHUNKS_PER_REGION],
}

pub struct RegionChunk {
    desc: ChunkDescription,
}

pub struct ChunkDescription {
    ranges: SmallVec<[Range; 4]>,
}

#[derive(Debug)]
struct Range {
    lower: GlobalSliceIndex,
    upper: GlobalSliceIndex,
    ty: RangeType,
}

enum RangeType {
    Solid(BlockType),
    HeightMap {
        under: BlockType,
        surface: BlockType,
        // TODO store u8/u16 relative to range minimum to save space
        height_map: ChunkHeightMap,
    },
}

grid_declare!(struct ChunkHeightMap<ChunkHeightMapImpl, i32>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    1
);

impl Regions {
    pub fn new(params: &PlanetParams) -> Self {
        Regions {
            params: params.clone(),
            regions: Vec::with_capacity(64),
        }
    }

    // TODO result for out of range
    pub async fn get_or_create(
        &mut self,
        location: RegionLocation,
        generator: Arc<Generator>,
    ) -> &Region {
        match self.region_index(location) {
            Ok(idx) => &self.regions[idx].1,
            Err(idx) => {
                debug!("creating new region"; "region" => ?location);
                let region = Region::create(location, generator, &self.params).await;
                self.regions.insert(idx, (location, region));
                &self.regions[idx].1
            }
        }
    }

    pub fn get_existing(&self, region: RegionLocation) -> Option<&Region> {
        self.region_index(region)
            .ok()
            .map(|idx| &self.regions[idx].1)
    }

    fn region_index(&self, region: RegionLocation) -> Result<usize, usize> {
        self.regions.binary_search_by_key(&region, |(pos, _)| *pos)
    }
}

impl Region {
    async fn create(
        coords: RegionLocation,
        generator: Arc<Generator>,
        params: &PlanetParams,
    ) -> Self {
        // using a log_scope here causes a nested panic, possibly due to dropping the scope multiple
        // times?
        debug!("creating region"; "region" => ?coords);

        let region = (coords.0 as f64, coords.1 as f64);
        let height_scale = params.height_scale as f64;

        // initialize chunk descriptions
        let mut chunks: [MaybeUninit<RegionChunk>; CHUNKS_PER_REGION] =
            unsafe { MaybeUninit::uninit().assume_init() };

        let handle = tokio::runtime::Handle::current();
        futures::future::join_all((0..CHUNKS_PER_REGION).map(|idx| {
            let generator = generator.clone();

            // cant pass a ptr across threads but you can an integer :^)
            // the array is stack allocated and we dont leave this function while this closure is
            // alive so this pointer is safe to use.
            let this_chunk = chunks[idx].as_mut_ptr() as usize;
            handle.spawn(async move {
                let chunk = RegionChunk::new(idx, region, generator, height_scale);

                // safety: each task has a single index in the chunk array
                unsafe {
                    let this_chunk = this_chunk as *mut RegionChunk;
                    this_chunk.write(chunk);
                }
            })
        }))
        .await;

        // safety: all chunks have been initialized
        let chunks: [RegionChunk; CHUNKS_PER_REGION] = unsafe { std::mem::transmute(chunks) };

        Region { chunks }
    }

    fn chunk_index(chunk: ChunkLocation) -> usize {
        let ChunkLocation(x, y) = chunk;
        let x = x.rem_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let y = y.rem_euclid(CHUNKS_PER_REGION_SIDE.as_i32());

        (x + (y * CHUNKS_PER_REGION_SIDE.as_i32())) as usize
    }

    pub fn chunk(&self, chunk: ChunkLocation) -> &RegionChunk {
        let idx = Self::chunk_index(chunk);
        debug_assert!(idx < self.chunks.len(), "bad idx {}", idx);
        &self.chunks[idx]
    }
}

impl RegionChunk {
    fn new(
        chunk_idx: usize,
        (rx, ry): (f64, f64),
        generator: Arc<Generator>,
        height_scale: f64,
    ) -> Self {
        const PER_BLOCK: f64 = 1.0 / (CHUNKS_PER_REGION_SIDE.as_f64() + CHUNK_SIZE.as_f64());

        let chunk_idx = chunk_idx as i32;
        let cx = chunk_idx % CHUNKS_PER_REGION_SIDE.as_i32();
        let cy = chunk_idx / CHUNKS_PER_REGION_SIDE.as_i32();
        let mut ranges = SmallVec::new();

        // get height for each surface block in chunk
        let mut height_map = ChunkHeightMap::default();
        let (mut min_height, mut max_height) = (i32::MAX, i32::MIN);
        for (i, (by, bx)) in (0..CHUNK_SIZE.as_i32())
            .cartesian_product(0..CHUNK_SIZE.as_i32())
            .enumerate()
        {
            let nx = rx + (((cx * CHUNK_SIZE.as_i32()) + bx) as f64 * PER_BLOCK);
            let ny = ry + (((cy * CHUNK_SIZE.as_i32()) + by) as f64 * PER_BLOCK);
            let height = map_range((-1.0, 1.0), (0.0, 1.0), generator.sample((nx, ny)));

            // convert height map float into block coords
            let block_height = (height * height_scale) as i32;

            height_map[i] = block_height;

            min_height = min_height.min(block_height);
            max_height = max_height.max(block_height);
        }

        ranges.push(Range::new(
            min_height,
            max_height,
            RangeType::HeightMap {
                height_map,
                under: BlockType::Dirt,
                surface: BlockType::Grass,
            },
        ));

        // everything below is stone, everything above is air
        ranges.push(Range::new(
            i32::MIN,
            min_height,
            RangeType::Solid(BlockType::Stone),
        ));
        ranges.push(Range::new(
            max_height,
            i32::MAX,
            RangeType::Solid(BlockType::Air),
        ));

        // TODO depends on many local parameters e.g. biome, humidity

        trace!("generated region chunk"; "chunk" => ?(cx, cy), "region" => ?(rx, ry));

        RegionChunk {
            desc: ChunkDescription::new(ranges),
        }
    }

    pub fn description(&self) -> &ChunkDescription {
        &self.desc
    }
}

impl ChunkDescription {
    fn new(mut ranges: SmallVec<[Range; 4]>) -> Self {
        ranges.sort_unstable_by_key(|r: &Range| r.lower);

        // ensure no overlap
        let mut last_upper = i32::MIN;
        for range in &ranges {
            debug_assert!(
                last_upper == range.lower.slice() && range.upper > range.lower,
                "last={}, this={:?}",
                last_upper,
                range
            );
            last_upper = range.upper.slice();
        }

        ChunkDescription { ranges }
    }

    pub fn query_block(&self, block: BlockPosition) -> BlockType {
        let range = self
            .ranges
            .iter()
            .find(|range| block.z() < range.upper)
            .unwrap_or_else(|| panic!("block {:?} matches no range", block));

        debug_assert!(block.z() >= range.lower);

        match &range.ty {
            RangeType::Solid(bt) => *bt,
            RangeType::HeightMap {
                under,
                surface,
                height_map,
            } => {
                let height = SliceIndex::new(height_map[&[block.x() as i32, block.y() as i32, 0]]);
                match block.z().cmp(&height) {
                    Ordering::Less => *under,
                    Ordering::Equal => *surface,
                    Ordering::Greater => BlockType::Air,
                }
            }
        }
    }

    pub fn apply_to_slab(&self, slab_idx: SlabIndex, slab: &mut SlabGrid) {
        let from_slice = slab_idx.as_i32() * SLAB_SIZE.as_i32();
        let to_slice = from_slice + SLAB_SIZE.as_i32();

        let mut current_range = None;
        let mut current_idx = 0;

        let mut next_range = |z: GlobalSliceIndex| match current_range {
            Some(max) if z < max => &self.ranges[current_idx],
            _ => {
                let (range_idx, range) = self
                    .ranges
                    .iter()
                    .enumerate()
                    .skip(current_idx)
                    .find(|(_, range)| z < range.upper)
                    .unwrap_or_else(|| panic!("slice {:?} matches no range", z));

                current_range = Some(range.upper);
                current_idx = range_idx;
                range
            }
        };

        for (z_global, z_local) in (from_slice..to_slice)
            .map(GlobalSliceIndex::new)
            .zip(LocalSliceIndex::range())
        {
            let range = next_range(z_global);
            let slice = {
                let (from, to) = slab.slice_range(z_local.slice_unsigned());
                &mut slab.array_mut()[from..to]
            };
            match &range.ty {
                RangeType::Solid(bt) => {
                    slice.iter_mut().for_each(|b| b.ty = *bt);
                }
                RangeType::HeightMap {
                    under,
                    surface,
                    height_map,
                } => {
                    for (i, (y, x)) in (0..CHUNK_SIZE.as_i32())
                        .cartesian_product(0..CHUNK_SIZE.as_i32())
                        .enumerate()
                    {
                        let height = SliceIndex::new(height_map[&[x, y, 0]]);
                        let bt = match z_global.cmp(&height) {
                            Ordering::Less => *under,
                            Ordering::Equal => *surface,
                            Ordering::Greater => BlockType::Air,
                        };

                        slice[i].ty = bt;
                    }
                }
            }
        }
    }
}

impl Range {
    fn new(min: i32, max: i32, ty: RangeType) -> Self {
        Range {
            lower: SliceIndex::new(min),
            upper: SliceIndex::new(max),
            ty,
        }
    }
}

impl From<ChunkLocation> for RegionLocation {
    fn from(chunk: ChunkLocation) -> Self {
        let x = chunk.0.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let y = chunk.1.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        RegionLocation(x, y)
    }
}

impl Debug for RangeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RangeType::Solid(bt) => write!(f, "solid {:?}", bt),
            RangeType::HeightMap { under, surface, .. } => {
                write!(f, "height map, {:?} below, {:?} above", under, surface)
            }
        }
    }
}

slog_value_debug!(RegionLocation);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_region() {
        assert_eq!(
            RegionLocation::from(ChunkLocation(-2, 1)),
            RegionLocation(-1, 0)
        );

        assert_eq!(
            RegionLocation::from(ChunkLocation(-CHUNKS_PER_REGION_SIDE.as_i32() - 1, -2)),
            RegionLocation(-2, -1)
        );
    }

    #[test]
    fn chunk_index() {
        assert_eq!(
            Region::chunk_index(ChunkLocation(0, 2)),
            CHUNKS_PER_REGION_SIDE.as_usize() * 2
        );

        assert_eq!(Region::chunk_index(ChunkLocation(3, 0)), 3);

        assert_eq!(
            Region::chunk_index(ChunkLocation(3 + (CHUNKS_PER_REGION_SIDE.as_i32() * 3), 0)),
            3
        );

        let idx = Region::chunk_index(ChunkLocation(
            CHUNKS_PER_REGION_SIDE.as_i32() - 1,
            CHUNKS_PER_REGION_SIDE.as_i32() - 2,
        ));
        assert_eq!(idx, 55);
        assert_eq!(Region::chunk_index(ChunkLocation(-1, -2)), idx);
    }
}
