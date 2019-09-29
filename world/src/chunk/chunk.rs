use std::cell::Cell;
use std::convert::TryFrom;
use std::ops::Shl;

use crate::block::BlockType;
use crate::coordinate::world::{Block, BlockCoord, ChunkPosition, SliceBlock, SliceIndex};
use crate::grid::{Grid, GridImpl};
use crate::grid_declare;
use crate::navigation::Navigation;
use crate::slice::{unflatten_index, Slice, SliceMut};

pub type ChunkId = u64;

// reexport
pub use crate::coordinate::world::CHUNK_SIZE;

pub const BLOCK_COUNT_CHUNK: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
pub const BLOCK_COUNT_SLICE: usize = CHUNK_SIZE * CHUNK_SIZE;

grid_declare!(struct ChunkGrid<ChunkGridImpl, BlockType>, CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE);

pub struct Chunk {
    /// unique for each chunk
    pos: ChunkPosition,
    blocks: ChunkGrid,
    dirty: Cell<bool>,
    nav: Navigation,
}

impl Chunk {
    pub fn empty<P: Into<ChunkPosition>>(pos: P) -> Self {
        Self::new(pos.into(), ChunkGrid::default())
    }

    /// Called by ChunkBuilder when terrain has been finalized
    pub(crate) fn new(pos: ChunkPosition, blocks: ChunkGrid) -> Self {
        let nav = Navigation::from_chunk(&blocks);
        let mut chunk = Self {
            pos,
            blocks,
            dirty: Cell::new(true),
            nav,
        };

        chunk.terrain_changed();

        chunk
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
        let SliceIndex(index) = index.into();
        let (from, to) = ChunkGrid::slice_range(index);
        SliceMut::new(&mut (*self.blocks)[from..to])
    }

    pub fn slice<S: Into<SliceIndex>>(&self, index: S) -> Slice {
        let SliceIndex(index) = index.into();
        let (from, to) = ChunkGrid::slice_range(index);
        Slice::new(&(*self.blocks)[from..to])
    }

    pub fn slices(&self) -> impl Iterator<Item = Slice> {
        (0i32..ChunkGrid::slice_count()).map(move |z| self.slice(z))
    }

    pub fn blocks(&self) -> Blocks {
        Blocks {
            chunk: self,
            current_slice: (SliceIndex(0), self.slice(0)),
            idx: 0,
        }
    }

    //    pub fn set_block<B: Into<Block>>(&mut self, pos: B, block: BlockType) {
    //        // TODO allow negative slice indices
    //        let Block(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = pos.into();
    //        self.blocks[&[i32::from(x), i32::from(y), z]] = block;
    //    }
    //
    pub fn get_block<B: Into<Block>>(&self, pos: B) -> BlockType {
        // TODO allow negative slice indices
        let Block(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = pos.into();
        self.blocks[&[i32::from(x), i32::from(y), z]]
    }

    /// Call when terrain has changed
    pub fn terrain_changed(&mut self) {
        // TODO update navigation

        // mark as dirty
        self.invalidate()
    }

    pub fn navigation(&self) -> &Navigation {
        &self.nav
    }
}

pub struct Blocks<'a> {
    chunk: &'a Chunk,
    current_slice: (SliceIndex, Slice<'a>),
    idx: usize,
}

impl<'a> Iterator for Blocks<'a> {
    type Item = (Block, BlockType);

    fn next(&mut self) -> Option<Self::Item> {
        let b = loop {
            match self.current_slice.1.get(self.idx) {
                Some(b) => break b,
                None => {
                    // next slice
                    let SliceIndex(idx) = self.current_slice.0 + 1;
                    if idx >= CHUNK_SIZE as i32 {
                        return None;
                    }
                    let next_slice = self.chunk.slice(idx);
                    self.current_slice = (SliceIndex(idx), next_slice);
                    self.idx = 0;
                    continue;
                }
            };
        };

        let SliceBlock(x, y) = unflatten_index(self.idx);
        let block_pos = Block(x, y, self.current_slice.0);

        self.idx += 1;

        Some((block_pos, *b))
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::{Chunk, ChunkBuilder, BLOCK_COUNT_SLICE};
    use crate::BLOCK_COUNT_CHUNK;

    #[test]
    fn chunk_ops() {
        // TODO immutable too
        let chunk = ChunkBuilder::new()
            .apply(|c| {
                // a bit on slice 0
                for i in 0_u16..3 {
                    c.set_block((i, i, 0), BlockType::Dirt);
                }
            })
            .set_block((2, 3, 1), BlockType::Dirt)
            .build((0, 0));

        // slice 1
        assert_eq!(chunk.get_block((2, 3, 1)), BlockType::Dirt);

        // collect slice
        let slice: Vec<BlockType> = chunk.slice(0).iter().map(|b| *b).collect();
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

    #[test]
    fn blocks() {
        let c = Chunk::empty((0, 0));
        let mut b = c.blocks();
        assert_eq!(b.next(), Some(((0, 0, 0).into(), BlockType::Air)));
        assert_eq!(b.next(), Some(((1, 0, 0).into(), BlockType::Air)));
        assert_eq!(b.next(), Some(((2, 0, 0).into(), BlockType::Air)));

        let rest: Vec<_> = b.collect();
        assert_eq!(rest.len(), BLOCK_COUNT_CHUNK - 3);
    }
}
