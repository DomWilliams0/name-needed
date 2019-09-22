use std::cell::Cell;
use std::convert::TryFrom;
use std::ops::Shl;

use crate::block::Block;
use crate::slice::{Slice, SliceMut};

pub type SliceIndex = i32;
pub type Coordinate = u32;
pub type ChunkPosition = (Coordinate, Coordinate);
pub type ChunkId = u64;

pub const MIN_SLICE: SliceIndex = std::i32::MIN;
pub const MAX_SLICE: SliceIndex = std::i32::MAX;

const SIZE: usize = 16;
pub const CHUNK_SIZE: u32 = SIZE as u32;

pub const BLOCK_COUNT_CHUNK: usize = SIZE * SIZE * SIZE;
pub const BLOCK_COUNT_SLICE: usize = SIZE * SIZE;

pub struct Chunk {
    /// unique for each chunk
    pos: ChunkPosition,
    blocks: [Block; BLOCK_COUNT_CHUNK],
    dirty: Cell<bool>,
}

impl Chunk {
    pub fn empty(pos: ChunkPosition) -> Self {
        Self {
            pos,
            blocks: [Block::Air; BLOCK_COUNT_CHUNK],
            dirty: Cell::new(true),
        }
    }

    pub fn pos(&self) -> ChunkPosition {
        self.pos
    }

    pub fn id(&self) -> ChunkId {
        let (x, y) = self.pos;
        (u64::from(x)).shl(32) | u64::from(y)
    }

    /// Clears dirty bit before returning it
    pub fn dirty(&self) -> bool {
        self.dirty.replace(false)
    }

    pub fn invalidate(&self) {
        self.dirty.set(true)
    }

    pub fn slice_mut(&mut self, index: SliceIndex) -> SliceMut {
        let (from, to) = self.slice_range(index);
        SliceMut::new(&mut self.blocks[from..to])
    }

    pub fn slice(&self, index: SliceIndex) -> Slice {
        let (from, to) = self.slice_range(index);
        Slice::new(&self.blocks[from..to])
    }

    fn slice_range(&self, index: SliceIndex) -> (usize, usize) {
        // TODO allow negative slice indices
        let index = usize::try_from(index).unwrap();
        let offset = index * BLOCK_COUNT_SLICE;
        (offset, offset + BLOCK_COUNT_SLICE)
    }

    pub fn set_block(&mut self, x: Coordinate, y: Coordinate, z: SliceIndex, block: Block) {
        self.blocks[Chunk::index(x, y, z)] = block;
    }

    pub fn get_block(&self, x: Coordinate, y: Coordinate, z: SliceIndex) -> Block {
        self.blocks[Chunk::index(x, y, z)]
    }

    fn index(x: Coordinate, y: Coordinate, z: SliceIndex) -> usize {
        let x = usize::try_from(x).unwrap();
        let y = usize::try_from(y).unwrap();
        let z = usize::try_from(z).unwrap();
        (z * BLOCK_COUNT_SLICE) + (y * SIZE) + x
    }
}

#[cfg(test)]
mod tests {
    use crate::block::Block;
    use crate::chunk::{Chunk, BLOCK_COUNT_SLICE, SIZE};

    #[test]
    fn chunk_ops() {
        // TODO immutable too
        let mut chunk = Chunk::empty((0, 0));

        // slice 0
        for i in 0u32..3 {
            chunk.set_block(i, i, 0, Block::Dirt);
        }

        // slice 1
        chunk.set_block(2, 3, 1, Block::Dirt);

        // collect slice
        let slice: Vec<Block> = chunk.slice_mut(0).iter().map(|b| *b).collect();
        assert_eq!(slice.len(), BLOCK_COUNT_SLICE); // ensure exact length
        assert_eq!(slice.iter().filter(|b| **b != Block::Air).count(), 3); // ensure exact number of filled blocks

        // ensure each exact coord was filled
        assert_eq!(chunk.get_block(0, 0, 0), Block::Dirt);
        assert_eq!(chunk.get_block(1, 1, 0), Block::Dirt);
        assert_eq!(chunk.get_block(2, 2, 0), Block::Dirt);
    }

    #[test]
    fn index() {
        // slices should be contiguous
        assert_eq!(Chunk::index(0, 0, 0), 0);
        assert_eq!(Chunk::index(1, 0, 0), 1);
        assert_eq!(Chunk::index(2, 0, 0), 2);

        assert_eq!(Chunk::index(0, 1, 0), SIZE);
        assert_eq!(Chunk::index(1, 1, 0), SIZE + 1);
        assert_eq!(Chunk::index(2, 1, 0), SIZE + 2);

        assert_eq!(Chunk::index(0, 0, 1), BLOCK_COUNT_SLICE);
        assert_eq!(Chunk::index(1, 0, 1), BLOCK_COUNT_SLICE + 1);
    }

    #[test]
    fn chunk_id() {
        let id1 = Chunk::empty((0, 0)).id();
        let id2 = Chunk::empty((0, 1)).id();
        let id3 = Chunk::empty((1, 0)).id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }
}
