use std::ops::{Deref, DerefMut};

use crate::block::BlockType;
use crate::coordinate::world::{BlockCoord, SliceBlock, CHUNK_SIZE};

pub struct Slice<'a> {
    slice: &'a [BlockType],
}

pub struct SliceMut<'a> {
    slice: &'a mut [BlockType],
}

impl<'a> Slice<'a> {
    pub fn new(slice: &'a [BlockType]) -> Self {
        Self { slice }
    }

    pub fn non_air_blocks(&self) -> impl Iterator<Item = (SliceBlock, &BlockType)> {
        self.slice
            .iter()
            .enumerate()
            .filter(|(_i, &b)| b != BlockType::Air)
            .map(|(i, b)| {
                let pos = unflatten_index(i);
                (pos, b)
            })
    }

    pub fn blocks(&self) -> impl Iterator<Item = (SliceBlock, &BlockType)> {
        self.slice.iter().enumerate().map(|(i, b)| {
            let pos = unflatten_index(i);
            (pos, b)
        })
    }
}

impl<'a> Deref for Slice<'a> {
    type Target = [BlockType];

    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

// -------

impl<'a> SliceMut<'a> {
    pub fn new(slice: &'a mut [BlockType]) -> Self {
        Self { slice }
    }

    pub fn set_block<P: Into<SliceBlock>>(&mut self, pos: P, block: BlockType) {
        let index = flatten_coords(pos.into());
        self.slice[index] = block;
    }
}

impl<'a> Deref for SliceMut<'a> {
    type Target = [BlockType];

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
        BlockCoord((index % CHUNK_SIZE as usize) as u16),
        BlockCoord((index / CHUNK_SIZE as usize) as u16),
    )
}

fn flatten_coords(block: SliceBlock) -> usize {
    let SliceBlock(BlockCoord(x), BlockCoord(y)) = block;
    ((y * CHUNK_SIZE as u16) + x) as usize
}

#[cfg(test)]
mod tests {
    use crate::chunk::CHUNK_SIZE;

    use super::*;

    #[test]
    fn unflatten_slice_index() {
        assert!(CHUNK_SIZE >= 3);

        assert_eq!(unflatten_index(0), (0, 0).into());
        assert_eq!(unflatten_index(1), (1, 0).into());
        assert_eq!(unflatten_index(2), (2, 0).into());

        let size = CHUNK_SIZE as usize;
        assert_eq!(unflatten_index(size + 0), (0, 1).into());
        assert_eq!(unflatten_index(size + 1), (1, 1).into());
        assert_eq!(unflatten_index(size + 2), (2, 1).into());
    }
}
