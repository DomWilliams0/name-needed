use std::cell::Cell;
use std::convert::TryFrom;
use std::ops::Shl;

use crate::block::BlockType;
use crate::coordinate::world::{Block, BlockCoord, ChunkPosition, SliceIndex};
use crate::grid::{Dims, Grid};
use crate::slice::{Slice, SliceMut};

pub type ChunkId = u64;

const SIZE: usize = 16;
pub const CHUNK_SIZE: u32 = SIZE as u32;

pub const BLOCK_COUNT_CHUNK: usize = SIZE * SIZE * SIZE;
pub const BLOCK_COUNT_SLICE: usize = SIZE * SIZE;

struct ChunkGridImpl;

impl Dims for ChunkGridImpl {
    fn dims() -> &'static [i32; 3] {
        &[SIZE as i32, SIZE as i32, SIZE as i32]
    }
}

type ChunkGrid = Grid<BlockType, ChunkGridImpl>;

pub struct Chunk {
    /// unique for each chunk
    pos: ChunkPosition,
    blocks: ChunkGrid,
    dirty: Cell<bool>,
}

impl Chunk {
    pub fn empty<P: Into<ChunkPosition>>(pos: P) -> Self {
        Self {
            pos: pos.into(),
            blocks: ChunkGrid::new(),
            dirty: Cell::new(true),
        }
    }

    pub fn pos(&self) -> ChunkPosition {
        self.pos
    }

    pub fn id(&self) -> ChunkId {
        let ChunkPosition(x, y) = self.pos;
        (u64::try_from(x).unwrap()).shl(32) | u64::try_from(y).unwrap()
    }

    /// Clears dirty bit before returning it
    pub fn dirty(&self) -> bool {
        self.dirty.replace(false)
    }

    pub fn invalidate(&self) {
        self.dirty.set(true)
    }

    pub fn slice_mut<S: Into<SliceIndex>>(&mut self, index: S) -> SliceMut {
        let (from, to) = self.slice_range(index.into());
        SliceMut::new(&mut (*self.blocks)[from..to])
    }

    pub fn slice<S: Into<SliceIndex>>(&self, index: S) -> Slice {
        let (from, to) = self.slice_range(index.into());
        Slice::new(&(*self.blocks)[from..to])
    }

    fn slice_range(&self, index: SliceIndex) -> (usize, usize) {
        // TODO allow negative slice indices
        let SliceIndex(index) = index;
        let index = usize::try_from(index).expect("negative slices not implemented yet");
        let offset = index * BLOCK_COUNT_SLICE;
        (offset, offset + BLOCK_COUNT_SLICE)
    }

    pub fn set_block<B: Into<Block>>(&mut self, pos: B, block: BlockType) {
        // TODO allow negative slice indices
        let Block(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = pos.into();
        self.blocks[&[i32::from(x), i32::from(y), z]] = block;
    }

    pub fn get_block<B: Into<Block>>(&self, pos: B) -> BlockType {
        // TODO allow negative slice indices
        let Block(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = pos.into();
        self.blocks[&[i32::from(x), i32::from(y), z]]
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::{Chunk, BLOCK_COUNT_SLICE};
    use crate::coordinate::world::{Block, BlockCoord, ChunkPosition, SliceIndex};

    #[test]
    fn chunk_ops() {
        // TODO immutable too
        let mut chunk = Chunk::empty((0, 0));

        // slice 0
        for i in 0u16..3 {
            chunk.set_block((i, i, 0), BlockType::Dirt);
        }

        // slice 1
        chunk.set_block((2, 3, 1), BlockType::Dirt);
        assert_eq!(chunk.get_block((2, 3, 1)), BlockType::Dirt);

        // collect slice
        let slice: Vec<BlockType> = chunk.slice_mut(0).iter().map(|b| *b).collect();
        assert_eq!(slice.len(), BLOCK_COUNT_SLICE); // ensure exact length
        assert_eq!(slice.iter().filter(|b| **b != BlockType::Air).count(), 3); // ensure exact number of filled blocks

        // ensure each exact coord was filled
        assert_eq!(chunk.get_block((0, 0, 0)), BlockType::Dirt);
        assert_eq!(chunk.get_block((1, 1, 0)), BlockType::Dirt);
        assert_eq!(chunk.get_block((2, 2, 0)), BlockType::Dirt);
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
