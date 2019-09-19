use std::convert::TryFrom;

use crate::block::Block;
use crate::slice::{Slice, SliceMut};

pub type SliceIndex = i32;
pub type Coordinate = u32;

const SIZE: usize = 16;
pub const CHUNK_SIZE: u32 = SIZE as u32;

pub const BLOCK_COUNT_CHUNK: usize = SIZE * SIZE * SIZE;
pub const BLOCK_COUNT_SLICE: usize = SIZE * SIZE;

pub struct Chunk {
    pos: (Coordinate, Coordinate),
    blocks: [Block; BLOCK_COUNT_CHUNK],
}

impl Chunk {
    pub fn empty(pos: (Coordinate, Coordinate)) -> Self {
        Self {
            pos,
            blocks: [Block::Air; BLOCK_COUNT_CHUNK],
        }
    }

    pub fn pos(&self) -> (Coordinate, Coordinate) {
        self.pos
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
}
