use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};

use crate::block::Block;
use crate::chunk::{Coordinate, CHUNK_SIZE};

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

    pub fn set_block(&mut self, x: Coordinate, y: Coordinate, block: Block) {
        let index = flatten_coords((x, y));
        self.slice[index] = block;
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

pub fn unflatten_index(index: usize) -> (Coordinate, Coordinate) {
    let index = Coordinate::try_from(index).unwrap();
    (index % CHUNK_SIZE, index / CHUNK_SIZE)
}

fn flatten_coords((x, y): (Coordinate, Coordinate)) -> usize {
    let x = usize::try_from(x).unwrap();
    let y = usize::try_from(y).unwrap();
    (y * CHUNK_SIZE as usize) + x
}

#[cfg(test)]
mod tests {
    use crate::chunk::CHUNK_SIZE;

    use super::*;

    #[test]
    fn unflatten_slice_index() {
        assert!(CHUNK_SIZE >= 3);

        assert_eq!(unflatten_index(0), (0, 0));
        assert_eq!(unflatten_index(1), (1, 0));
        assert_eq!(unflatten_index(2), (2, 0));

        let size = CHUNK_SIZE as usize;
        assert_eq!(unflatten_index(size + 0), (0, 1));
        assert_eq!(unflatten_index(size + 1), (1, 1));
        assert_eq!(unflatten_index(size + 2), (2, 1));
    }
}
