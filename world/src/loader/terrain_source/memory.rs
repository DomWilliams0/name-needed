use std::collections::HashMap;

use misc::*;
use unit::world::{BlockPosition, ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition};

use crate::chunk::slab::Slab;
use crate::chunk::RawChunkTerrain;
use crate::loader::terrain_source::TerrainSourceError;
use crate::{BlockType, WorldContext};

/// Used for testing
pub struct MemoryTerrainSource<C: WorldContext> {
    chunk_map: HashMap<ChunkLocation, RawChunkTerrain<C>>,
    bounds: (ChunkLocation, ChunkLocation),
}

impl<C: WorldContext> MemoryTerrainSource<C> {
    pub fn from_chunks<P: Into<ChunkLocation>, T: Into<(P, RawChunkTerrain<C>)>>(
        chunks: impl Iterator<Item = T>,
    ) -> Result<Self, TerrainSourceError> {
        let size = chunks.size_hint().1.unwrap_or(8);
        let mut chunk_map = HashMap::with_capacity(size);

        for it in chunks {
            let (chunk, terrain) = it.into();
            let chunk = chunk.into();

            // assert placeholder slabs are really placeholders
            if cfg!(debug_assertions) {
                for (slab, idx) in terrain.slabs_from_bottom() {
                    if slab.is_placeholder() {
                        for (_, slice) in slab.slices_from_bottom() {
                            assert!(
                                slice.all_blocks_are(C::BlockType::AIR),
                                "non air blocks in \"placeholder\" slab {:?}",
                                SlabLocation::new(idx, chunk)
                            );
                        }
                    }
                }
            }

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

    pub fn all_slabs(&self) -> impl Iterator<Item = SlabLocation> + '_ {
        self.chunk_map.iter().flat_map(|(chunk, terrain)| {
            let (min, max) = terrain.slab_range();
            (min.as_i32()..=max.as_i32()).map(move |slab| chunk.get_slab(slab))
        })
    }

    pub fn get_slab_copy(&self, slab: SlabLocation) -> Result<Slab<C>, TerrainSourceError> {
        let slab = self
            .chunk_map
            .get(&slab.chunk)
            .and_then(|terrain| terrain.copy_slab(slab.slab))
            .ok_or(TerrainSourceError::SlabOutOfBounds(slab))?;

        Ok(slab)
    }

    pub fn get_ground_level(
        &self,
        block: WorldPosition,
    ) -> Result<GlobalSliceIndex, TerrainSourceError> {
        self.chunk_map
            .get(&block.into())
            .and_then(|terrain| {
                let block = BlockPosition::from(block);
                terrain.find_ground_level(block.into(), None, None)
            })
            .map(|block| block.z())
            .ok_or(TerrainSourceError::BlockOutOfBounds(block))
    }

    /// Bounding box, inclusive
    pub fn world_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        self.bounds
    }

    /// Checks chunk bounds only, assume infinite depth
    pub fn is_in_bounds(&self, slab: SlabLocation) -> bool {
        let (min, max) = self.bounds;
        (min.0..=max.0).contains(&slab.chunk.0) && (min.1..=max.1).contains(&slab.chunk.1)
    }
}
