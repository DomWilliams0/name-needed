use std::collections::HashMap;

use common::*;
use unit::world::{ChunkLocation, SlabLocation};

use crate::chunk::slab::Slab;
use crate::chunk::RawChunkTerrain;
use crate::loader::terrain_source::{PreprocessedTerrain, TerrainSource, TerrainSourceError};

/// Used for testing
#[derive(Clone)]
pub struct MemoryTerrainSource {
    /// Each slab is removed from terrain as it's loaded
    chunk_map: HashMap<ChunkLocation, RawChunkTerrain>,
    bounds: (ChunkLocation, ChunkLocation),
}

impl MemoryTerrainSource {
    pub fn from_chunks<P: Into<ChunkLocation>, C: Into<(P, RawChunkTerrain)>>(
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

        if !chunk_map.contains_key(&ChunkLocation(0, 0)) {
            return Err(TerrainSourceError::MissingCentreChunk);
        }

        // calculate world bounds
        let bounds = match (
            chunk_map.keys().map(|c| c.0).minmax(),
            chunk_map.keys().map(|c| c.1).minmax(),
        ) {
            (MinMaxResult::MinMax(min_x, max_x), MinMaxResult::MinMax(min_y, max_y)) => {
                (ChunkLocation(min_x, min_y), ChunkLocation(max_x, max_y))
            }
            // must have single chunk
            _ => (ChunkLocation(0, 0), ChunkLocation(0, 0)),
        };

        Ok(Self { chunk_map, bounds })
    }

    pub fn all_slabs(&mut self) -> impl Iterator<Item = SlabLocation> + '_ {
        self.chunk_map.iter().flat_map(|(chunk, terrain)| {
            let (min, max) = terrain.slab_range();
            (min.as_i32()..=max.as_i32()).map(move |slab| chunk.get_slab(slab))
        })
    }
}

impl TerrainSource for MemoryTerrainSource {
    fn world_bounds(&self) -> &(ChunkLocation, ChunkLocation) {
        &self.bounds
    }

    fn preprocess(
        &self,
        _slab: SlabLocation,
    ) -> Box<dyn FnOnce() -> Result<Box<dyn PreprocessedTerrain>, TerrainSourceError>> {
        // nothing to do
        Box::new(|| Ok(Box::new(())))
    }

    fn load_slab(
        &mut self,
        slab: SlabLocation,
        _: Box<dyn PreprocessedTerrain>,
    ) -> Result<Slab, TerrainSourceError> {
        let slab = self
            .chunk_map
            .get(&slab.chunk)
            .and_then(|terrain| terrain.copy_slab(slab.slab))
            .ok_or(TerrainSourceError::OutOfBounds(slab))?;

        Ok(slab)
    }

    /*    fn preprocess(
            &self,
            _: ChunkLocation,
        ) -> Box<dyn FnOnce() -> Result<Box<dyn PreprocessedTerrain>, TerrainSourceError>> {
        }

        fn load_chunk(
            &mut self,
            chunk: ChunkLocation,
            _: Box<dyn PreprocessedTerrain>,
        ) -> Result<RawChunkTerrain, TerrainSourceError> {
            self.chunk_map
                .get_mut(&chunk)
                .ok_or(TerrainSourceError::OutOfBounds)
                .and_then(|terrain| terrain.take().ok_or(TerrainSourceError::Duplicate(chunk)))
        }
    */
}

impl PreprocessedTerrain for () {
    fn into_raw_terrain(self: Box<Self>) -> RawChunkTerrain {
        unreachable!()
    }
}
