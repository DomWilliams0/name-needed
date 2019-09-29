use std::cell::Cell;
use std::mem::MaybeUninit;

use nd_iter::iter_3d;

use crate::block::BlockType;
use crate::coordinate::world::{Block, BlockCoord, SliceIndex};
use crate::slice::SliceMut;
use crate::{Chunk, ChunkGrid, ChunkPosition};

pub struct ChunkBuilder {
    blocks: ChunkGrid,
}

pub struct ChunkBuilderApply {
    inner: Cell<ChunkBuilder>,
}

impl ChunkBuilder {
    pub fn new() -> Self {
        Self {
            blocks: ChunkGrid::default(),
        }
    }

    pub fn set_block<B: Into<Block>>(mut self, pos: B, block: BlockType) -> Self {
        // TODO allow negative slice indices
        // TODO copied!
        let Block(BlockCoord(x), BlockCoord(y), SliceIndex(z)) = pos.into();
        self.blocks[&[i32::from(x), i32::from(y), z]] = block;
        self
    }

    pub fn fill_slice<S: Into<SliceIndex>>(mut self, slice: S, block: BlockType) -> Self {
        let SliceIndex(index) = slice.into();
        let (from, to) = ChunkGrid::slice_range(index);

        let blocks: &mut [BlockType] = &mut *self.blocks;

        for b in &mut blocks[from..to] {
            *b = block;
        }

        self
    }

    pub fn fill_range<F, T, B>(mut self, from: F, to: T, block: B) -> Self
    where
        F: Into<Block>,
        T: Into<Block>,
        B: Fn((i32, i32, i32)) -> Option<BlockType>,
    {
        let [fx, fy, fz]: [i32; 3] = from.into().into();
        let [tx, ty, tz]: [i32; 3] = to.into().into();

        for (x, y, z) in iter_3d(fx..tx, fy..ty, fz..tz) {
            let b = block((x, y, z));
            if let Some(block) = b {
                self.blocks[&[x, y, z]] = block;
            }
        }

        self
    }

    pub fn with_slice<S, F>(mut self, slice: S, mut f: F) -> Self
    where
        S: Into<SliceIndex>,
        F: FnMut(SliceMut),
    {
        // TODO copied from chunk.slice_mut
        let SliceIndex(index) = slice.into();
        let (from, to) = ChunkGrid::slice_range(index);
        let slice = SliceMut::new(&mut (*self.blocks)[from..to]);
        f(slice);
        self
    }

    pub fn apply<F: FnMut(&mut ChunkBuilderApply)>(self, mut f: F) -> Self {
        let mut apply = ChunkBuilderApply {
            inner: Cell::new(self),
        };
        f(&mut apply);
        apply.inner.into_inner()
    }

    pub fn build<P: Into<ChunkPosition>>(self, pos: P) -> Chunk {
        Chunk::new(pos.into(), self.blocks)
    }
}

impl ChunkBuilderApply {
    pub fn set_block<B: Into<Block>>(&mut self, pos: B, block: BlockType) -> &mut Self {
        // swap out inner with dAnGeRoUs uNdEfInEd bEhAvIoUr
        #[allow(clippy::uninit_assumed_init)]
        let dummy_uninit = unsafe { MaybeUninit::uninit().assume_init() };
        let chunk_builder = self.inner.replace(dummy_uninit);

        // self.inner is currently undefined

        // process and get new builder
        let new = chunk_builder.set_block(pos, block);

        self.inner.set(new);

        // tada, bad uninitialized memory has been overwritten

        self
    }
}

impl Default for ChunkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::ChunkBuilder;

    #[test]
    fn fill_slice() {
        let c = ChunkBuilder::new()
            .fill_slice(0, BlockType::Grass)
            .build((0, 0));

        assert!(c.slice(0).iter().all(|b| *b == BlockType::Grass));
        assert!(c.slice(1).iter().all(|b| *b == BlockType::Air));
    }

    #[test]
    fn set_block() {
        let c = ChunkBuilder::new()
            .set_block((2, 2, 1), BlockType::Stone)
            .build((0, 0));

        assert_eq!(c.get_block((2, 2, 1)), BlockType::Stone);
    }

    #[test]
    fn apply() {
        let c = ChunkBuilder::new()
            .apply(|c| {
                c.set_block((1, 1, 1), BlockType::Grass);
                c.set_block((1, 2, 1), BlockType::Grass);
            })
            .build((0, 0));

        assert_eq!(c.get_block((1, 1, 1)), BlockType::Grass);
        assert_eq!(c.get_block((1, 2, 1)), BlockType::Grass);
    }

    #[test]
    fn fill_range() {
        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (3, 3, 3), |_| Some(BlockType::Stone))
            .build((0, 0));

        assert_eq!(
            c.blocks().filter(|(_, b)| *b == BlockType::Stone).count(),
            3 * 3 * 3
        );

        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (3, 3, 3), |_| None)
            .build((0, 0));

        assert_eq!(
            c.blocks().filter(|(_, b)| *b == BlockType::Stone).count(),
            0
        );

        let c = ChunkBuilder::new()
            .fill_range((0, 3, 3), (8, 4, 4), |(x, _y, _z)| {
                if x == 0 {
                    Some(BlockType::Dirt)
                } else {
                    None
                }
            })
            .build((0, 0));

        assert_eq!(c.blocks().filter(|(_, b)| *b == BlockType::Dirt).count(), 1);
        assert_eq!(c.get_block((0, 3, 3)), BlockType::Dirt);

        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (10, 0, 0), |_| Some(BlockType::Stone))
            .build((0, 0));
        assert_eq!(
            c.blocks().filter(|(_, b)| *b == BlockType::Stone).count(),
            0
        );

        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (10, 1, 1), |_| Some(BlockType::Stone))
            .build((0, 0));
        assert_eq!(
            c.blocks().filter(|(_, b)| *b == BlockType::Stone).count(),
            10
        );
    }
}
