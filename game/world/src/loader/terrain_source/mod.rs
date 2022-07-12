use std::collections::HashSet;
use std::sync::Arc;

use common::parking_lot::RwLock;
use common::*;
#[cfg(feature = "procgen")]
pub use generate::GeneratedTerrainSource;
pub use memory::MemoryTerrainSource;
use unit::world::{
    ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition, WorldPositionRange,
};

use crate::chunk::slab::Slab;
use crate::loader::WorldTerrainUpdate;
use crate::WorldContext;

#[cfg(feature = "procgen")]
mod generate;
mod memory;

#[derive(Debug, Error)]
pub enum TerrainSourceError {
    #[error("There are no chunks")]
    NoChunks,

    #[error("Missing mandatory (0, 0) chunk")]
    MissingCentreChunk,

    #[error("Chunk {0:?} redefined")]
    Duplicate(ChunkLocation),

    #[error("Requested slab {0} is out of bounds")]
    SlabOutOfBounds(SlabLocation),

    #[error("Requested block {0} is out of bounds")]
    BlockOutOfBounds(WorldPosition),

    #[error("Received signal to bail")]
    Bailed,

    #[error("Async task failed to complete: {0}")]
    Async(#[from] tokio::task::JoinError),
}

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub enum TerrainSource<C: WorldContext> {
    Memory(Arc<RwLock<MemoryTerrainSource<C>>>),
    #[cfg(feature = "procgen")]
    Generated(GeneratedTerrainSource),
}

unsafe impl<C: WorldContext> Send for TerrainSource<C> {}
unsafe impl<C: WorldContext> Sync for TerrainSource<C> {}

#[cfg(feature = "procgen")]
pub struct BlockDetails {
    pub biome_choices: SmallVec<[(procgen::BiomeType, f32); 4]>,
    pub coastal_proximity: f64,
    pub base_elevation: f64,
    pub moisture: f64,
    pub temperature: f64,
    /// (region location, dirty string representation of features affecting this block)
    pub region: Option<(procgen::RegionLocation, SmallVec<[String; 4]>)>,
}

pub struct GeneratedSlab<C: WorldContext> {
    pub terrain: Slab<C>,
    pub entities: Vec<C::GeneratedEntityDesc>,
}

impl<C: WorldContext> From<MemoryTerrainSource<C>> for TerrainSource<C> {
    fn from(src: MemoryTerrainSource<C>) -> Self {
        Self::Memory(Arc::new(RwLock::new(src)))
    }
}

#[cfg(feature = "procgen")]
impl<C: WorldContext> From<GeneratedTerrainSource> for TerrainSource<C> {
    fn from(src: GeneratedTerrainSource) -> Self {
        Self::Generated(src)
    }
}

impl<C: WorldContext> TerrainSource<C> {
    pub async fn prepare_for_chunks(&self, range: (ChunkLocation, ChunkLocation)) {
        match self {
            TerrainSource::Memory(_) => {}
            #[cfg(feature = "procgen")]
            TerrainSource::Generated(src) => src.planet().prepare_for_chunks(range).await,
        }
    }

    pub async fn load_slab(
        &self,
        slab: SlabLocation,
    ) -> Result<GeneratedSlab<C>, TerrainSourceError> {
        match self {
            TerrainSource::Memory(src) => src
                .read()
                .get_slab_copy(slab)
                .map(GeneratedSlab::with_terrain),
            #[cfg(feature = "procgen")]
            TerrainSource::Generated(src) => src.load_slab(slab).await,
        }
    }

    /// z is ignored in input
    pub async fn get_ground_level(
        &self,
        block: WorldPosition,
    ) -> Result<GlobalSliceIndex, TerrainSourceError> {
        match self {
            TerrainSource::Memory(src) => src.read().get_ground_level(block),
            #[cfg(feature = "procgen")]
            TerrainSource::Generated(src) => src
                .get_ground_level(block)
                .await
                .ok_or(TerrainSourceError::BlockOutOfBounds(block)),
        }
    }

    #[cfg(feature = "procgen")]
    pub async fn query_block(&self, block: WorldPosition) -> Option<BlockDetails> {
        match self {
            TerrainSource::Memory(_) => None,
            TerrainSource::Generated(src) => {
                src.planet()
                    .query_block(block)
                    .await
                    .map(|result| BlockDetails {
                        biome_choices: result
                            .biome_choices
                            .choices()
                            .map(|(b, w)| (b.ty(), w.value()))
                            .collect(),
                        coastal_proximity: result.coastal_proximity,
                        base_elevation: result.base_elevation,
                        moisture: result.moisture,
                        temperature: result.temperature,
                        region: result.region,
                    })
            }
        }
    }

    pub async fn feature_boundaries_in_range(
        &self,
        chunks: impl Iterator<Item = ChunkLocation>,
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        per_point: impl FnMut(usize, WorldPosition),
    ) {
        match self {
            TerrainSource::Memory(_) => {}
            #[cfg(feature = "procgen")]
            TerrainSource::Generated(planet) => {
                planet
                    .planet()
                    .feature_boundaries_in_range(chunks, z_range, per_point)
                    .await
            }
        }
    }

    pub async fn steal_queued_block_updates(&self, out: &mut HashSet<WorldTerrainUpdate<C>>) {
        match self {
            TerrainSource::Memory(_) => {}
            #[cfg(feature = "procgen")]
            TerrainSource::Generated(planet) => {
                let len_before = out.len();
                planet
                    .planet()
                    .steal_world_updates(|updates| {
                        out.extend(updates.map(|(pos, block)| {
                            WorldTerrainUpdate::new(WorldPositionRange::with_single(pos), block.ty)
                        }));
                    })
                    .await;
                let n = out.len() - len_before;
                if n > 0 {
                    debug!(
                        "collected {count} block updates from planet generation",
                        count = n
                    );
                }
            }
        }
    }
}

impl<C: WorldContext> GeneratedSlab<C> {
    pub fn with_terrain(terrain: Slab<C>) -> Self {
        Self {
            terrain,
            entities: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use crate::chunk::RawChunkTerrain;
    use crate::helpers::DummyWorldContext;
    use crate::loader::terrain_source::memory::MemoryTerrainSource;

    use super::*;

    #[test]
    fn invalid() {
        let no_chunks: Vec<(ChunkLocation, _)> = vec![];
        let empty = MemoryTerrainSource::<DummyWorldContext>::from_chunks(no_chunks.into_iter());
        assert!(matches!(empty.err().unwrap(), TerrainSourceError::NoChunks));

        let random = MemoryTerrainSource::<DummyWorldContext>::from_chunks(once((
            (5, 5),
            RawChunkTerrain::default(),
        )));
        assert!(matches!(
            random.err().unwrap(),
            TerrainSourceError::MissingCentreChunk
        ));
    }

    #[test]
    fn bounds() {
        let just_one = MemoryTerrainSource::<DummyWorldContext>::from_chunks(once((
            (0, 0),
            RawChunkTerrain::default(),
        )))
        .unwrap();
        assert_eq!(
            just_one.world_bounds(),
            (ChunkLocation(0, 0), ChunkLocation(0, 0))
        );

        // cheap check to tests bounds
        assert!(!just_one.is_in_bounds(ChunkLocation(1, 1).get_slab(0)));

        // make sure impl fails too
        assert!(just_one.get_slab_copy(SlabLocation::new(0, (0, 0))).is_ok());

        assert!(matches!(
            just_one.get_slab_copy(SlabLocation::new(0, (1, 1))),
            Err(TerrainSourceError::SlabOutOfBounds(_))
        ));
        let sparse = MemoryTerrainSource::<DummyWorldContext>::from_chunks(
            vec![
                ((0, 0), RawChunkTerrain::default()),
                ((2, 5), RawChunkTerrain::default()),
                ((1, 6), RawChunkTerrain::default()),
                ((-5, -4), RawChunkTerrain::default()),
                ((-8, -2), RawChunkTerrain::default()),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(
            sparse.world_bounds(),
            (ChunkLocation(-8, -4), ChunkLocation(2, 6))
        );
    }
}
