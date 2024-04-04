use std::collections::HashSet;
use std::sync::Arc;

pub use memory::MemoryTerrainSource;
use misc::parking_lot::RwLock;
use misc::*;
use unit::world::{ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition};

use crate::chunk::slab::Slab;
#[cfg(feature = "worldprocgen")]
use crate::context::GeneratedTerrainSource;
use crate::loader::WorldTerrainUpdate;
use crate::WorldContext;

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

    #[error("Async task failed to complete: {0}")]
    Async(#[from] tokio::task::JoinError),
}

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub enum TerrainSource<C: WorldContext> {
    Memory(Arc<MemoryTerrainSource<C>>),
    #[cfg(feature = "worldprocgen")]
    Generated(C::GeneratedTerrainSource),
}

unsafe impl<C: WorldContext> Send for TerrainSource<C> {}
unsafe impl<C: WorldContext> Sync for TerrainSource<C> {}

pub struct GeneratedSlab<C: WorldContext> {
    pub terrain: Slab<C>,
    pub entities: Vec<C::GeneratedEntityDesc>,
}

impl<C: WorldContext> From<MemoryTerrainSource<C>> for TerrainSource<C> {
    fn from(src: MemoryTerrainSource<C>) -> Self {
        Self::Memory(Arc::new(src))
    }
}

impl<C: WorldContext> TerrainSource<C> {
    pub async fn prepare_for_chunks(&self, range: (ChunkLocation, ChunkLocation)) {
        match self {
            TerrainSource::Memory(_) => {}
            #[cfg(feature = "worldprocgen")]
            TerrainSource::Generated(src) => src.prepare_for_chunks(range).await,
        }
    }

    pub async fn load_slab(
        &self,
        slab: SlabLocation,
    ) -> Result<GeneratedSlab<C>, TerrainSourceError> {
        match self {
            TerrainSource::Memory(src) => src.get_slab_copy(slab).map(GeneratedSlab::with_terrain),
            #[cfg(feature = "worldprocgen")]
            TerrainSource::Generated(src) => {
                // TODO handle wrapping of slabs around planet boundaries
                src.generate_slab(slab)
                    .await
                    .ok_or(TerrainSourceError::SlabOutOfBounds(slab))
            }
        }
    }

    /// z is ignored in input
    pub async fn get_ground_level(
        &self,
        block: WorldPosition,
    ) -> Result<GlobalSliceIndex, TerrainSourceError> {
        match self {
            TerrainSource::Memory(src) => src.get_ground_level(block),
            #[cfg(feature = "worldprocgen")]
            TerrainSource::Generated(src) => src
                .find_ground_level(block)
                .await
                .ok_or(TerrainSourceError::BlockOutOfBounds(block)),
        }
    }

    #[cfg(feature = "worldprocgen")]
    pub async fn query_block(&self, block: WorldPosition) -> Option<C::GeneratedBlockDetails> {
        match self {
            TerrainSource::Memory(_) => None,
            TerrainSource::Generated(src) => src.query_block(block).await,
        }
    }

    pub async fn feature_boundaries_in_range(
        &self,
        chunks: &[ChunkLocation],
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        output: &mut Vec<(usize, WorldPosition)>,
    ) {
        match self {
            TerrainSource::Memory(_) => {}
            #[cfg(feature = "worldprocgen")]
            TerrainSource::Generated(src) => {
                src.feature_boundaries_in_range(chunks, z_range, output)
                    .await
            }
        }
    }

    pub async fn steal_queued_block_updates(&self, out: &mut HashSet<WorldTerrainUpdate<C>>) {
        match self {
            TerrainSource::Memory(_) => {}
            #[cfg(feature = "worldprocgen")]
            TerrainSource::Generated(src) => {
                let len_before = out.len();
                src.steal_queued_block_updates(out).await;
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

    pub fn world_boundary(&self) -> (ChunkLocation, ChunkLocation) {
        match self {
            TerrainSource::Memory(src) => src.world_bounds(),
            #[cfg(feature = "worldprocgen")]
            TerrainSource::Generated(_) => (ChunkLocation(0, 0), ChunkLocation::MAX), // planet chunks dont go negative
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

    use crate::chunk::SlabStorage;
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
            SlabStorage::default(),
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
            SlabStorage::default(),
        )))
        .unwrap();
        assert_eq!(
            just_one.world_bounds(),
            (ChunkLocation(0, 0), ChunkLocation(0, 0))
        );

        // cheap check to tests bounds
        assert!(!just_one.is_in_bounds(ChunkLocation(1, 1)));

        // make sure impl fails too
        assert!(just_one.get_slab_copy(SlabLocation::new(0, (0, 0))).is_ok());

        assert!(matches!(
            just_one.get_slab_copy(SlabLocation::new(0, (1, 1))),
            Err(TerrainSourceError::SlabOutOfBounds(_))
        ));
        let sparse = MemoryTerrainSource::<DummyWorldContext>::from_chunks(
            vec![
                ((0, 0), SlabStorage::default()),
                ((2, 5), SlabStorage::default()),
                ((1, 6), SlabStorage::default()),
                ((-5, -4), SlabStorage::default()),
                ((-8, -2), SlabStorage::default()),
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
