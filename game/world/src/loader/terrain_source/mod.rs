use crate::chunk::RawChunkTerrain;
use common::*;
use unit::world::{ChunkLocation, SlabLocation};

#[derive(Debug, Error)]
pub enum TerrainSourceError {
    #[error("There are no chunks")]
    NoChunks,

    #[error("Missing mandatory (0, 0) chunk")]
    MissingCentreChunk,

    #[error("Chunk {0:?} redefined")]
    Duplicate(ChunkLocation),

    #[error("Requested slab {0} is out of bounds")]
    OutOfBounds(SlabLocation),

    #[error("Received signal to bail")]
    Bailed,
}

pub trait PreprocessedTerrain: Send {
    fn into_raw_terrain(self: Box<Self>) -> RawChunkTerrain;
}

pub trait TerrainSource: Send {
    /// Bounding box, inclusive
    fn world_bounds(&self) -> &(ChunkLocation, ChunkLocation);

    fn preprocess(
        &self,
        slab: SlabLocation,
    ) -> Box<dyn FnOnce() -> Result<Box<dyn PreprocessedTerrain>, TerrainSourceError>>;

    fn load_slab(
        &mut self,
        slab: SlabLocation,
        preprocess_result: Box<dyn PreprocessedTerrain>,
    ) -> Result<Slab, TerrainSourceError>;

    /// Checks chunk bounds only, assume infinite depth
    fn is_in_bounds(&self, slab: SlabLocation) -> bool {
        let (min, max) = self.world_bounds();
        (min.0..=max.0).contains(&slab.chunk.0) && (min.1..=max.1).contains(&slab.chunk.1)
    }
}

mod generate;
mod memory;
use crate::chunk::slab::Slab;
pub use generate::GeneratedTerrainSource;
pub use memory::MemoryTerrainSource;

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut just_one =
            MemoryTerrainSource::from_chunks(once(((0, 0), RawChunkTerrain::default()))).unwrap();
        assert_eq!(
            *just_one.world_bounds(),
            (ChunkLocation(0, 0), ChunkLocation(0, 0))
        );

        // cheap check to tests bounds
        assert!(!just_one.is_in_bounds(ChunkLocation(1, 1).get_slab(0)));

        // make sure impl fails too
        assert!(just_one
            .load_slab(SlabLocation::new(0, (0, 0)), Box::new(()))
            .is_ok());

        assert!(matches!(
            just_one.load_slab(SlabLocation::new(0, (1, 1)), Box::new(())),
            Err(TerrainSourceError::OutOfBounds(_))
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
            *sparse.world_bounds(),
            (ChunkLocation(-8, -4), ChunkLocation(2, 6))
        );
    }
}
