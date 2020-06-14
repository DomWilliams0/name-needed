use crate::chunk::RawChunkTerrain;
use unit::world::ChunkPosition;

#[derive(Debug)]
pub enum TerrainSourceError {
    NoChunks,
    MissingCentreChunk,
    Duplicate(ChunkPosition),
    OutOfBounds,
}

pub trait PreprocessedTerrain: Send {
    fn into_raw_terrain(self: Box<Self>) -> RawChunkTerrain;
}

pub trait TerrainSource: Send {
    /// Bounding box, not necessarily full
    fn world_bounds(&self) -> &(ChunkPosition, ChunkPosition);

    fn all_chunks(&mut self) -> Vec<ChunkPosition>; // TODO gross

    fn preprocess(
        &self,
        chunk: ChunkPosition,
    ) -> Box<dyn FnOnce() -> Result<Box<dyn PreprocessedTerrain>, TerrainSourceError>>;

    fn load_chunk(
        &mut self,
        chunk: ChunkPosition,
        preprocess_result: Box<dyn PreprocessedTerrain>,
    ) -> Result<RawChunkTerrain, TerrainSourceError>;

    /*
    fn unload_chunk(
        &mut self,
        chunk: ChunkPosition,
        terrain: RawChunkTerrain,
    ) -> Result<(), TerrainSourceError>;
    */

    fn is_in_bounds(&self, chunk: ChunkPosition) -> bool {
        let (min, max) = self.world_bounds();
        (min.0..=max.0).contains(&chunk.0) && (min.1..=max.1).contains(&chunk.1)
    }
}

mod generate;
mod memory;
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
        let no_chunks: Vec<(ChunkPosition, RawChunkTerrain)> = vec![];
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
            (ChunkPosition(0, 0), ChunkPosition(0, 0))
        );

        // cheap check to tests bounds
        assert!(!just_one.is_in_bounds(ChunkPosition(1, 1)));

        // make sure impl fails too
        assert!(just_one
            .load_chunk(ChunkPosition(0, 0), Box::new(()))
            .is_ok());
        assert_matches!(
            just_one
                .load_chunk(ChunkPosition(1, 1), Box::new(()))
                .err()
                .unwrap(),
            TerrainSourceError::OutOfBounds
        );

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
            (ChunkPosition(-8, -4), ChunkPosition(2, 6))
        );
    }
}
