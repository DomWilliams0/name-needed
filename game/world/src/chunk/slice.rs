use std::ops::{Deref, DerefMut, Index, IndexMut};

use unit::dim::CHUNK_SIZE;
use unit::world::{BlockCoord, SliceBlock};

use crate::block::{Block, BlockType};

const DUMMY_SLICE_BLOCKS: [Block; CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize()] =
    [Block::default_const(); CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize()];

#[derive(Clone)]
pub struct Slice<'a> {
    slice: &'a [Block],
}

pub struct SliceMut<'a> {
    slice: &'a mut [Block],
}

impl<'a> Slice<'a> {
    pub fn new(slice: &'a [Block]) -> Self {
        Self { slice }
    }

    pub fn dummy() -> Slice<'static> {
        Slice {
            slice: &DUMMY_SLICE_BLOCKS,
        }
    }

    pub fn non_air_blocks(&self) -> impl Iterator<Item = (usize, SliceBlock, &Block)> {
        self.filter_blocks(move |&b| b.block_type() != BlockType::Air)
    }

    pub fn slice(&self) -> &[Block] {
        self.slice
    }

    pub fn filter_blocks<F>(&self, f: F) -> impl Iterator<Item = (usize, SliceBlock, &Block)>
    where
        F: Fn(&Block) -> bool,
    {
        self.slice
            .iter()
            .enumerate()
            .filter(move |(_i, b)| f(b))
            .map(|(i, b)| {
                let pos = unflatten_index(i);
                (i, pos, b)
            })
    }

    pub fn all_blocks_are(&self, block_type: BlockType) -> bool {
        self.filter_blocks(move |&b| b.block_type() != block_type)
            .count()
            == 0
    }

    pub fn blocks(&self) -> impl Iterator<Item = (SliceBlock, &Block)> {
        self.slice.iter().enumerate().map(|(i, b)| {
            let pos = unflatten_index(i);
            (pos, b)
        })
    }

    pub fn index_unchecked(&self, idx: usize) -> &Block {
        debug_assert!(idx < self.slice.len());
        unsafe { self.slice.get_unchecked(idx) }
    }
}

impl<'a> Deref for Slice<'a> {
    type Target = [Block];

    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

// -------

impl<'a> SliceMut<'a> {
    pub fn new(slice: &'a mut [Block]) -> Self {
        Self { slice }
    }

    /// Must point to a slice of length CHUNK_SIZE * CHUNK_SIZE
    pub unsafe fn from_ptr(ptr: *mut Block) -> Self {
        let slice =
            std::slice::from_raw_parts_mut(ptr, CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize());
        Self::new(slice)
    }

    pub fn set_block<P, B>(&mut self, pos: P, block: B)
    where
        P: Into<SliceBlock>,
        B: Into<Block>,
    {
        let index = flatten_coords(pos.into());
        self.slice[index] = block.into();
    }

    pub fn fill<B>(&mut self, block: B)
    where
        B: Into<Block>,
    {
        let block = block.into();
        for b in self.slice.iter_mut() {
            *b = block;
        }
    }
}

impl Deref for SliceMut<'_> {
    type Target = [Block];

    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl DerefMut for SliceMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}

// -------

impl<I: Into<SliceBlock>> Index<I> for Slice<'_> {
    type Output = Block;

    fn index(&self, index: I) -> &Self::Output {
        &self.slice[flatten_coords(index.into())]
    }
}

impl<I: Into<SliceBlock>> Index<I> for SliceMut<'_> {
    type Output = Block;

    fn index(&self, index: I) -> &Self::Output {
        &self.slice[flatten_coords(index.into())]
    }
}

impl<I: Into<SliceBlock>> IndexMut<I> for SliceMut<'_> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.slice[flatten_coords(index.into())]
    }
}

// TODO make not pub
pub fn unflatten_index(index: usize) -> SliceBlock {
    SliceBlock(
        (index % CHUNK_SIZE.as_usize()) as BlockCoord,
        (index / CHUNK_SIZE.as_usize()) as BlockCoord,
    )
}

fn flatten_coords(block: SliceBlock) -> usize {
    let SliceBlock(x, y) = block;
    ((y * CHUNK_SIZE.as_block_coord()) + x) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unflatten_slice_index() {
        // these tests assume the chunk size is at least 3
        assert!(CHUNK_SIZE.as_i32() >= 3);

        assert_eq!(unflatten_index(0), (0, 0).into());
        assert_eq!(unflatten_index(1), (1, 0).into());
        assert_eq!(unflatten_index(2), (2, 0).into());

        let size = CHUNK_SIZE.as_usize();
        assert_eq!(unflatten_index(size), (0, 1).into());
        assert_eq!(unflatten_index(size + 1), (1, 1).into());
        assert_eq!(unflatten_index(size + 2), (2, 1).into());
    }
}
