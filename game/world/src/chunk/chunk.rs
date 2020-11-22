use std::ops::{Deref, DerefMut};

use common::*;

use unit::world::{BlockPosition, ChunkLocation, GlobalSliceIndex, WorldPosition};

use crate::block::BlockType;
use crate::chunk::slice::Slice;
use crate::chunk::terrain::{ChunkTerrain, RawChunkTerrain};
use crate::chunk::BaseTerrain;
use crate::navigation::WorldArea;
use crate::SliceRange;
use std::collections::HashMap;

pub type ChunkId = u64;

pub struct Chunk<D> {
    /// Unique for each chunk
    pos: ChunkLocation,

    terrain: ChunkTerrain,

    /// Sparse associated data with each block
    block_data: HashMap<BlockPosition, D>,
}

impl<D> Chunk<D> {
    pub fn empty<P: Into<ChunkLocation>>(pos: P) -> Self {
        let pos = pos.into();
        // TODO still does a lot of unnecessary initialization
        let terrain = ChunkTerrain::from_new_raw_terrain(RawChunkTerrain::default(), pos);
        Self::with_completed_terrain(pos, terrain)
    }

    /// Called by ChunkBuilder when terrain has been finalized
    pub(crate) fn with_completed_terrain(pos: ChunkLocation, terrain: ChunkTerrain) -> Self {
        Self {
            pos,
            terrain,
            block_data: HashMap::new(),
        }
    }

    pub const fn pos(&self) -> ChunkLocation {
        self.pos
    }

    pub fn id(&self) -> ChunkId {
        let ChunkLocation(x, y) = self.pos;
        (x as u64) << 32 | (y as u64)
    }

    pub fn get_block_type<B: Into<BlockPosition>>(&self, pos: B) -> Option<BlockType> {
        self.get_block(pos).map(|b| b.block_type())
    }

    pub(crate) fn area_for_block(&self, pos: WorldPosition) -> Option<WorldArea> {
        self.get_block(pos).map(|b| {
            let area_index = b.area_index();
            let block_pos: BlockPosition = pos.into();
            WorldArea {
                chunk: self.pos,
                slab: block_pos.z().slab_index(),
                area: area_index,
            }
        })
    }

    pub fn slice_range(
        &self,
        range: SliceRange,
    ) -> impl Iterator<Item = (GlobalSliceIndex, Slice)> {
        range
            .as_range()
            .map(move |i| self.slice(i).map(|s| (GlobalSliceIndex::new(i), s)))
            .skip_while(|s| s.is_none())
            .while_some()
    }

    pub fn slice_or_dummy(&self, slice: GlobalSliceIndex) -> Slice {
        #[allow(clippy::redundant_closure)]
        self.slice(slice).unwrap_or_else(|| Slice::dummy())
    }

    pub fn associated_block_data(&self, pos: BlockPosition) -> Option<&D> {
        self.block_data.get(&pos)
    }

    pub fn set_associated_block_data(&mut self, pos: BlockPosition, data: D) -> Option<D> {
        self.block_data.insert(pos, data)
    }

    pub fn remove_associated_block_data(&mut self, pos: BlockPosition) -> Option<D> {
        self.block_data.remove(&pos)
    }

    pub(crate) fn swap_with(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.block_data, &mut other.block_data)
    }
}

impl<D> Deref for Chunk<D> {
    type Target = ChunkTerrain;

    fn deref(&self) -> &Self::Target {
        &self.terrain
    }
}

impl<D> DerefMut for Chunk<D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terrain
    }
}

impl<D> BaseTerrain for Chunk<D> {
    fn raw_terrain(&self) -> &RawChunkTerrain {
        self.terrain.raw_terrain()
    }

    fn raw_terrain_mut(&mut self) -> &mut RawChunkTerrain {
        self.terrain.raw_terrain_mut()
    }
}

#[cfg(test)]
mod tests {
    use unit::world::GlobalSliceIndex;

    use crate::block::BlockType;
    use crate::chunk::terrain::BaseTerrain;
    use crate::chunk::{Chunk, ChunkBuilder};
    use unit::dim::CHUNK_SIZE;

    #[test]
    fn chunk_ops() {
        // check setting and getting blocks works
        let chunk = ChunkBuilder::new()
            .apply(|c| {
                // a bit on slice 0
                for i in 0..3 {
                    c.set_block((i, i, 0), BlockType::Dirt);
                }
            })
            .set_block((2, 3, 1), BlockType::Dirt)
            .into_inner();

        // slice 1 was filled
        assert_eq!(chunk.get_block_type((2, 3, 1)), Some(BlockType::Dirt));

        // collect slice
        let slice: Vec<BlockType> = chunk
            .slice(GlobalSliceIndex::new(0))
            .unwrap()
            .iter()
            .map(|b| b.block_type())
            .collect();
        assert_eq!(slice.len(), CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize()); // ensure exact length
        assert_eq!(slice.iter().filter(|b| **b != BlockType::Air).count(), 3); // ensure exact number of filled blocks

        // ensure each exact coord was filled
        assert_eq!(chunk.get_block_type((0, 0, 0)), Some(BlockType::Dirt));
        assert_eq!(chunk.get_block_type((1, 1, 0)), Some(BlockType::Dirt));
        assert_eq!(chunk.get_block_type((2, 2, 0)), Some(BlockType::Dirt));
    }

    #[test]
    fn chunk_id() {
        // check chunk ids are unique
        let id1 = Chunk::<()>::empty((0, 0)).id();
        let id2 = Chunk::<()>::empty((0, 1)).id();
        let id3 = Chunk::<()>::empty((1, 0)).id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn blocks() {
        // check individual block collection is ordered as intended
        let c = Chunk::<()>::empty((0, 0));
        let mut blocks = Vec::new();
        c.blocks(&mut blocks);
        let mut b = blocks.into_iter();
        assert_eq!(
            b.next().map(|(p, b)| (p, b.block_type())),
            Some(((0, 0, 0).into(), BlockType::Air))
        );
        assert_eq!(
            b.next().map(|(p, b)| (p, b.block_type())),
            Some(((1, 0, 0).into(), BlockType::Air))
        );
        assert_eq!(
            b.next().map(|(p, b)| (p, b.block_type())),
            Some(((2, 0, 0).into(), BlockType::Air))
        );
    }
}
