use std::cell::Cell;
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut, Shl};

use common::*;

use crate::area::WorldArea;
use crate::block::BlockType;
use crate::chunk::slab::{SlabIndex, SLAB_SIZE};
use crate::chunk::terrain::ChunkTerrain;
pub use unit::dim::CHUNK_SIZE;
use unit::world::{BlockPosition, ChunkPosition, WorldPosition};

pub type ChunkId = u64;

// reexport

pub const BLOCK_COUNT_SLICE: usize = CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize();

pub struct Chunk {
    /// unique for each chunk
    pos: ChunkPosition,

    terrain: ChunkTerrain,

    dirty: Cell<bool>,
    // nav: Navigation,
}

impl Chunk {
    pub fn empty<P: Into<ChunkPosition>>(pos: P) -> Self {
        Self::with_terrain(pos.into(), ChunkTerrain::default())
    }

    /// Called by ChunkBuilder when terrain has been finalized
    pub(crate) fn with_terrain(pos: ChunkPosition, mut terrain: ChunkTerrain) -> Self {
        debug!("discovering areas for chunk {:?}", pos);
        terrain.discover_areas(pos);
        terrain.init_occlusion();

        Self {
            pos,
            terrain,
            dirty: Cell::new(true),
        }
    }

    pub const fn pos(&self) -> ChunkPosition {
        self.pos
    }

    pub(crate) fn slab_pos(chunk_pos: ChunkPosition, slab: SlabIndex) -> WorldPosition {
        let mut pos = WorldPosition::from(chunk_pos);
        pos.2 = slab * SLAB_SIZE.as_i32();
        pos
    }

    pub fn id(&self) -> ChunkId {
        let ChunkPosition(x, y) = self.pos;
        (u64::try_from(x).unwrap()).shl(32) | u64::try_from(y).unwrap()
    }

    /// Clears dirty bit before returning it
    pub fn dirty(&self) -> bool {
        self.dirty.replace(false)
    }

    /// Sets dirty bit
    pub fn invalidate(&self) {
        self.dirty.set(true)
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
                slab: ChunkTerrain::slab_index_for_slice(block_pos.2),
                area: area_index,
            }
        })
    }
}

impl Deref for Chunk {
    type Target = ChunkTerrain;

    fn deref(&self) -> &Self::Target {
        &self.terrain
    }
}

impl DerefMut for Chunk {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terrain
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::{Chunk, ChunkBuilder, BLOCK_COUNT_SLICE};

    #[test]
    fn chunk_ops() {
        // check setting and getting blocks works
        let chunk = ChunkBuilder::new()
            .apply(|c| {
                // a bit on slice 0
                for i in 0_u16..3 {
                    c.set_block((i, i, 0), BlockType::Dirt);
                }
            })
            .set_block((2, 3, 1), BlockType::Dirt)
            .build((0, 0));

        // slice 1 was filled
        assert_eq!(chunk.get_block_type((2, 3, 1)), Some(BlockType::Dirt));

        // collect slice
        let slice: Vec<BlockType> = chunk
            .slice(0)
            .unwrap()
            .iter()
            .map(|b| b.block_type())
            .collect();
        assert_eq!(slice.len(), BLOCK_COUNT_SLICE); // ensure exact length
        assert_eq!(slice.iter().filter(|b| **b != BlockType::Air).count(), 3); // ensure exact number of filled blocks

        // ensure each exact coord was filled
        assert_eq!(chunk.get_block_type((0, 0, 0)), Some(BlockType::Dirt));
        assert_eq!(chunk.get_block_type((1, 1, 0)), Some(BlockType::Dirt));
        assert_eq!(chunk.get_block_type((2, 2, 0)), Some(BlockType::Dirt));
    }

    #[test]
    fn chunk_id() {
        // check chunk ids are unique
        let id1 = Chunk::empty((0, 0)).id();
        let id2 = Chunk::empty((0, 1)).id();
        let id3 = Chunk::empty((1, 0)).id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn blocks() {
        // check individual block collection is ordered as intended
        let c = Chunk::empty((0, 0));
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
