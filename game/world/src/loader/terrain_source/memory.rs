use std::collections::HashMap;

use common::*;
use unit::world::ChunkPosition;

use crate::chunk::RawChunkTerrain;
use crate::loader::terrain_source::{TerrainSource, TerrainSourceError};

/// Used for testing
pub struct MemoryTerrainSource {
    chunk_map: HashMap<ChunkPosition, RawChunkTerrain>,
    bounds: (ChunkPosition, ChunkPosition),
}

impl MemoryTerrainSource {
    pub fn from_chunks<P: Into<ChunkPosition>, C: Into<(P, RawChunkTerrain)>>(
        chunks: impl Iterator<Item = C>,
    ) -> Result<Self, TerrainSourceError> {
        let size = chunks.size_hint().1.unwrap_or(8);
        let mut chunk_map = HashMap::with_capacity(size);

        for it in chunks {
            let (chunk, terrain) = it.into();
            let chunk = chunk.into();
            if chunk_map.insert(chunk, terrain).is_some() {
                return Err(TerrainSourceError::Duplicate(chunk));
            }
        }

        if chunk_map.is_empty() {
            return Err(TerrainSourceError::NoChunks);
        }

        if !chunk_map.contains_key(&ChunkPosition(0, 0)) {
            return Err(TerrainSourceError::MissingCentreChunk);
        }

        // calculate world bounds
        let bounds = match (
            chunk_map.keys().map(|c| c.0).minmax(),
            chunk_map.keys().map(|c| c.1).minmax(),
        ) {
            (MinMaxResult::MinMax(min_x, max_x), MinMaxResult::MinMax(min_y, max_y)) => {
                (ChunkPosition(min_x, min_y), ChunkPosition(max_x, max_y))
            }
            // must have single chunk
            _ => (ChunkPosition(0, 0), ChunkPosition(0, 0)),
        };

        Ok(Self { chunk_map, bounds })
    }
}

impl TerrainSource for MemoryTerrainSource {
    fn world_bounds(&self) -> &(ChunkPosition, ChunkPosition) {
        &self.bounds
    }

    fn all_chunks(&mut self) -> Vec<ChunkPosition> {
        self.chunk_map.keys().copied().collect_vec()
    }

    fn load_chunk(&mut self, chunk: ChunkPosition) -> Result<RawChunkTerrain, TerrainSourceError> {
        self.chunk_map
            .get(&chunk)
            .ok_or(TerrainSourceError::OutOfBounds)
            .map(|terrain| terrain.clone()) // expensive but not too important for this
    }
}
