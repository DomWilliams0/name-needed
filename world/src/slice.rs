use std::ops::{Deref, DerefMut};

use crate::block::{Block, BlockHeight, BlockType};
use crate::coordinate::world::{BlockCoord, SliceBlock};
use crate::CHUNK_SIZE;

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

    pub fn non_air_blocks(&self) -> impl Iterator<Item = (SliceBlock, &Block)> {
        self.filter_blocks(move |&b| b.block_type != BlockType::Air)
    }

    pub fn filter_blocks<F>(&self, f: F) -> impl Iterator<Item = (SliceBlock, &Block)>
    where
        F: Fn(&Block) -> bool,
    {
        self.slice
            .iter()
            .enumerate()
            .filter(move |(_i, b)| f(b))
            .map(|(i, b)| {
                let pos = unflatten_index(i);
                (pos, b)
            })
    }

    pub fn all_blocks_are(&self, block_type: BlockType) -> bool {
        self.filter_blocks(move |&b| b.block_type != block_type)
            .count() == 0
    }

    pub fn blocks(&self) -> impl Iterator<Item = (SliceBlock, &Block)> {
        self.slice.iter().enumerate().map(|(i, b)| {
            let pos = unflatten_index(i);
            (pos, b)
        })
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

    pub fn set_block<P: Into<SliceBlock>>(&mut self, pos: P, block: BlockType) {
        self.set_block_with_height(pos, block, BlockHeight::default())
    }
    pub fn set_block_with_height<P: Into<SliceBlock>>(
        &mut self,
        pos: P,
        block: BlockType,
        height: BlockHeight,
    ) {
        let index = flatten_coords(pos.into());
        self.slice[index] = Block {
            block_type: block,
            height,
        }
    }
}

impl<'a> Deref for SliceMut<'a> {
    type Target = [Block];

    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<'a> DerefMut for SliceMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}

// -------

pub fn unflatten_index(index: usize) -> SliceBlock {
    SliceBlock(
        BlockCoord((index % CHUNK_SIZE.as_usize()) as u16),
        BlockCoord((index / CHUNK_SIZE.as_usize()) as u16),
    )
}

fn flatten_coords(block: SliceBlock) -> usize {
    let SliceBlock(BlockCoord(x), BlockCoord(y)) = block;
    ((y * CHUNK_SIZE.as_u16()) + x) as usize
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn unflatten_slice_index() {
        assert!(CHUNK_SIZE.as_i32() >= 3);

        assert_eq!(unflatten_index(0), (0, 0).into());
        assert_eq!(unflatten_index(1), (1, 0).into());
        assert_eq!(unflatten_index(2), (2, 0).into());

        let size = CHUNK_SIZE.as_usize();
        assert_eq!(unflatten_index(size + 0), (0, 1).into());
        assert_eq!(unflatten_index(size + 1), (1, 1).into());
        assert_eq!(unflatten_index(size + 2), (2, 1).into());
    }
}
