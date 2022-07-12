use std::ops::{Deref, DerefMut, Index, IndexMut};

use unit::world::CHUNK_SIZE;
use unit::world::{BlockCoord, SliceBlock};

use crate::block::Block;
use crate::{BlockType, WorldContext};
use common::Derivative;
use std::convert::TryInto;
use std::fmt::{Debug, Formatter};

pub(crate) const SLICE_SIZE: usize = CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize();

#[derive(Derivative)]
#[derivative(Clone(bound = ""), Copy(bound = ""))]
pub struct Slice<'a, C: WorldContext> {
    slice: &'a [Block<C>],
}

pub struct SliceMut<'a, C: WorldContext> {
    slice: &'a mut [Block<C>],
}

// TODO consider generalising Slice{,Mut,Owned} to hold other types than just Block e.g. opacity

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct SliceOwned<C: WorldContext> {
    slice: Box<[Block<C>; SLICE_SIZE]>,
}

impl<'a, C: WorldContext> Slice<'a, C> {
    pub fn new(slice: &'a [Block<C>]) -> Self {
        Self { slice }
    }

    pub fn dummy() -> Slice<'static, C> {
        Slice {
            slice: C::air_slice(),
        }
    }

    pub fn non_air_blocks(&self) -> impl Iterator<Item = (usize, SliceBlock, &Block<C>)> {
        self.filter_blocks(move |&b| !b.block_type().is_air())
    }

    pub fn slice(&self) -> &[Block<C>] {
        self.slice
    }

    pub fn filter_blocks<F>(&self, f: F) -> impl Iterator<Item = (usize, SliceBlock, &Block<C>)>
    where
        F: Fn(&Block<C>) -> bool,
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

    pub fn all_blocks_are(&self, block_type: C::BlockType) -> bool {
        self.filter_blocks(move |&b| b.block_type() != block_type)
            .count()
            == 0
    }

    pub fn blocks(&self) -> impl Iterator<Item = (SliceBlock, &Block<C>)> {
        self.slice.iter().enumerate().map(|(i, b)| {
            let pos = unflatten_index(i);
            (pos, b)
        })
    }

    pub fn index_unchecked(&self, idx: usize) -> &Block<C> {
        debug_assert!(idx < self.slice.len());
        unsafe { self.slice.get_unchecked(idx) }
    }

    pub fn to_owned(self) -> SliceOwned<C> {
        let slice = self.slice.try_into().expect("slice is the wrong length");
        SliceOwned {
            slice: Box::new(slice),
        }
    }

    pub fn into_iter(self) -> impl Iterator<Item = &'a Block<C>> {
        self.slice.iter()
    }
}

impl<'a, C: WorldContext> Deref for Slice<'a, C> {
    type Target = [Block<C>];

    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<C: WorldContext> SliceOwned<C> {
    pub fn borrow(&self) -> Slice<C> {
        Slice {
            slice: &*self.slice,
        }
    }
}

impl<'a, C: WorldContext> From<&'a SliceOwned<C>> for Slice<'a, C> {
    fn from(slice: &'a SliceOwned<C>) -> Self {
        Slice {
            slice: &*slice.slice,
        }
    }
}

impl<'a, C: WorldContext> From<SliceMut<'a, C>> for Slice<'a, C> {
    fn from(slice: SliceMut<'a, C>) -> Self {
        Slice { slice: slice.slice }
    }
}

impl<C: WorldContext> Debug for SliceOwned<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SliceOwned({} blocks)", self.slice.len())
    }
}

// -------

impl<'a, C: WorldContext> SliceMut<'a, C> {
    pub fn new(slice: &'a mut [Block<C>]) -> Self {
        Self { slice }
    }

    /// Must point to a slice of length CHUNK_SIZE * CHUNK_SIZE
    pub unsafe fn from_ptr(ptr: *mut Block<C>) -> Self {
        let slice =
            std::slice::from_raw_parts_mut(ptr, CHUNK_SIZE.as_usize() * CHUNK_SIZE.as_usize());
        Self::new(slice)
    }

    pub(crate) fn set_block<P>(&mut self, pos: P, block_type: C::BlockType) -> C::BlockType
    where
        P: Into<SliceBlock>,
    {
        let index = flatten_coords(pos.into());
        let b = &mut self.slice[index];

        let prev = b.block_type();
        *b = Block::with_block_type(block_type);
        prev
    }

    pub fn fill(&mut self, block: C::BlockType) {
        let block = Block::with_block_type(block);
        for b in self.slice.iter_mut() {
            *b = block;
        }
    }
}

impl<C: WorldContext> Deref for SliceMut<'_, C> {
    type Target = [Block<C>];

    fn deref(&self) -> &Self::Target {
        self.slice
    }
}

impl<C: WorldContext> DerefMut for SliceMut<'_, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice
    }
}

// -------

impl<I: Into<SliceBlock>, C: WorldContext> Index<I> for Slice<'_, C> {
    type Output = Block<C>;

    fn index(&self, index: I) -> &Self::Output {
        &self.slice[flatten_coords(index.into())]
    }
}

impl<I: Into<SliceBlock>, C: WorldContext> Index<I> for SliceMut<'_, C> {
    type Output = Block<C>;

    fn index(&self, index: I) -> &Self::Output {
        let i = flatten_coords(index.into());
        &self.slice[i]
    }
}

impl<I: Into<SliceBlock>, C: WorldContext> IndexMut<I> for SliceMut<'_, C> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.slice[flatten_coords(index.into())]
    }
}

// TODO make not pub
pub fn unflatten_index(index: usize) -> SliceBlock {
    SliceBlock::new_unchecked(
        (index % CHUNK_SIZE.as_usize()) as BlockCoord,
        (index / CHUNK_SIZE.as_usize()) as BlockCoord,
    )
}

fn flatten_coords(block: SliceBlock) -> usize {
    let (x, y) = block.xy();
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
