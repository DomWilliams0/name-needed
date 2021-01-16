use common::*;
use unit::world::{ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition};

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

#[derive(Clone)]
pub enum TerrainSource {
    Memory(Arc<RwLock<MemoryTerrainSource>>),
    Generated(GeneratedTerrainSource),
}

unsafe impl Send for TerrainSource {}
unsafe impl Sync for TerrainSource {}

impl From<MemoryTerrainSource> for TerrainSource {
    fn from(src: MemoryTerrainSource) -> Self {
        Self::Memory(Arc::new(RwLock::new(src)))
    }
}

impl From<GeneratedTerrainSource> for TerrainSource {
    fn from(src: GeneratedTerrainSource) -> Self {
        Self::Generated(src)
    }
}

impl TerrainSource {
    pub async fn prepare_for_chunks(&self, range: (ChunkLocation, ChunkLocation)) {
        match self {
            TerrainSource::Memory(_) => {}
            TerrainSource::Generated(src) => src.planet().prepare_for_chunks(range).await,
        }
    }

    pub async fn load_slab(&self, slab: SlabLocation) -> Result<Slab, TerrainSourceError> {
        match self {
            TerrainSource::Memory(src) => src.read().get_slab_copy(slab),
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
            TerrainSource::Generated(src) => Ok(src.get_ground_level(block).await),
        }
    }
}

mod generate;
mod memory;
use crate::chunk::slab::Slab;
use common::parking_lot::RwLock;
pub use generate::GeneratedTerrainSource;
pub use memory::MemoryTerrainSource;

use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::RawChunkTerrain;
    use crate::loader::terrain_source::memory::MemoryTerrainSource;
    use matches::assert_matches;
    use std::iter::once;

    #[test]
    fn invalid() {
        let no_chunks: Vec<(ChunkLocation, RawChunkTerrain)> = vec![];
        let empty = MemoryTerrainSource::from_chunks(no_chunks.into_iter());
        assert_matches!(empty.err().unwrap(), TerrainSourceError::NoChunks);

        let random = MemoryTerrainSource::from_chunks(once(((5, 5), RawChunkTerrain::default())));
        assert_matches!(
            random.err().unwrap(),
            TerrainSourceError::MissingCentreChunk
        );
    }

    #[test]
    fn bounds() {
        let just_one =
            MemoryTerrainSource::from_chunks(once(((0, 0), RawChunkTerrain::default()))).unwrap();
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
        let sparse = MemoryTerrainSource::from_chunks(
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
