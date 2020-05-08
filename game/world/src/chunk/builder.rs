use std::cell::Cell;
use std::mem;
use std::mem::MaybeUninit;

use nd_iter::iter_3d;

use unit::world::{BlockPosition, ChunkPosition, SliceIndex};

use crate::block::Block;
use crate::chunk::slice::SliceMut;
use crate::chunk::terrain::{RawChunkTerrain, SlabCreationPolicy};
use crate::chunk::BaseTerrain;

pub struct ChunkBuilder {
    terrain: RawChunkTerrain,
}

pub struct ChunkBuilderApply {
    inner: Cell<ChunkBuilder>,
}

impl ChunkBuilder {
    pub fn new() -> Self {
        Self {
            terrain: RawChunkTerrain::default(),
        }
    }

    /// Will create slabs as necessary
    pub fn set_block<P, B>(mut self, pos: P, block: B) -> Self
    where
        P: Into<BlockPosition>,
        B: Into<Block>,
    {
        self.terrain
            .set_block(pos, block, SlabCreationPolicy::CreateAll);
        self
    }

    pub fn fill_slice<S, B>(mut self, slice: S, block: B) -> Self
    where
        S: Into<SliceIndex>,
        B: Into<Block>,
    {
        // TODO create slice if missing
        if let Some(mut slice) = self.terrain.slice_mut(slice) {
            slice.fill(block);
        }

        self
    }

    pub fn fill_range<F, T, B, C>(mut self, from: F, to: T, mut block: C) -> Self
    where
        F: Into<BlockPosition>,
        T: Into<BlockPosition>,
        B: Into<Block>,
        C: FnMut((i32, i32, i32)) -> B,
    {
        let [fx, fy, fz]: [i32; 3] = from.into().into();
        let [tx, ty, tz]: [i32; 3] = to.into().into();

        for (x, y, z) in iter_3d(fx..tx, fy..ty, fz..tz) {
            let pos = (x, y, z);
            self = self.set_block(pos, block(pos));
        }

        self
    }

    pub fn with_slice<S, F>(mut self, slice: S, mut f: F) -> Self
    where
        S: Into<SliceIndex>,
        F: FnMut(SliceMut),
    {
        if let Some(slice) = self.terrain.slice_mut(slice) {
            f(slice);
        }

        self
    }

    pub fn apply<F: FnMut(&mut ChunkBuilderApply)>(self, mut f: F) -> Self {
        let mut apply = ChunkBuilderApply {
            inner: Cell::new(self),
        };
        f(&mut apply);
        apply.inner.into_inner()
    }

    pub fn build<P: Into<ChunkPosition>>(self, pos: P) -> ChunkDescriptor {
        ChunkDescriptor {
            terrain: self.terrain,
            chunk_pos: pos.into(),
        }
    }

    #[cfg(test)]
    pub fn into_inner(self) -> RawChunkTerrain {
        self.terrain
    }
}

impl ChunkBuilderApply {
    pub fn set_block<P, B>(&mut self, pos: P, block: B) -> &mut Self
    where
        P: Into<BlockPosition>,
        B: Into<Block>,
    {
        // swap out inner with dAnGeRoUs uNdEfInEd bEhAvIoUr - it's fine we put it back
        #[allow(clippy::uninit_assumed_init, invalid_value)]
        let dummy_uninit = unsafe { MaybeUninit::uninit().assume_init() };
        let chunk_builder = self.inner.replace(dummy_uninit);

        // self.inner is currently undefined

        // process and get new builder
        let new = chunk_builder.set_block(pos, block);

        // swap them back
        let dummy_uninit = self.inner.replace(new);

        // forget about the dummy, otherwise it will be dropped
        mem::forget(dummy_uninit);

        // tada, back to being safe
        self
    }
}

impl Default for ChunkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct ChunkDescriptor {
    pub terrain: RawChunkTerrain,
    pub chunk_pos: ChunkPosition,
}

impl Into<(ChunkPosition, RawChunkTerrain)> for ChunkDescriptor {
    fn into(self) -> (ChunkPosition, RawChunkTerrain) {
        (self.chunk_pos, self.terrain)
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::{BaseTerrain, ChunkBuilder};

    #[test]
    fn fill_slice() {
        // check that filling a slice with a block really does
        let c = ChunkBuilder::new()
            .fill_slice(0, BlockType::Grass)
            .into_inner();

        assert!(c.slice(0).unwrap().all_blocks_are(BlockType::Grass));
        assert!(c.slice(1).unwrap().all_blocks_are(BlockType::Air));
    }

    #[test]
    fn set_block() {
        // check setting a specific block works
        let c = ChunkBuilder::new()
            .set_block((2, 2, 1), BlockType::Stone)
            .set_block((3, 3, 3), BlockType::Grass)
            .into_inner();

        assert_eq!(
            c.get_block((2, 2, 1)).unwrap().block_type(),
            BlockType::Stone
        );

        assert_eq!(
            c.get_block((3, 3, 3)).unwrap().block_type(),
            BlockType::Grass
        );
    }

    #[test]
    fn apply() {
        // check that the apply helper works as intended
        let c = ChunkBuilder::new()
            .apply(|c| {
                c.set_block((1, 1, 1), BlockType::Grass);
                c.set_block((1, 2, 1), BlockType::Grass);
            })
            .into_inner();

        assert_eq!(
            c.get_block((1, 1, 1)).map(|b| b.block_type()),
            Some(BlockType::Grass)
        );
        assert_eq!(
            c.get_block((1, 2, 1)).map(|b| b.block_type()),
            Some(BlockType::Grass)
        );
    }

    #[test]
    fn fill_range() {
        // check that range filling works as intended
        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (3, 3, 3), |_| BlockType::Stone)
            .into_inner();
        let mut blocks = Vec::new();

        // expected to have filled 0-2 on all 3 dimensions
        assert_eq!(
            c.blocks(&mut blocks)
                .iter()
                .filter(|(_, b)| b.block_type() == BlockType::Stone)
                .count(),
            3 * 3 * 3
        );

        // annoyingly if any dimension has a width of 0, do nothing
        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (10, 0, 0), |_| BlockType::Stone)
            .into_inner();
        assert_eq!(
            c.blocks(&mut blocks)
                .iter()
                .filter(|(_, b)| b.block_type() == BlockType::Stone)
                .count(),
            0
        );

        // alternatively with a width of 1, work as intended
        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (10, 1, 1), |_| BlockType::Stone)
            .into_inner();
        assert_eq!(
            c.blocks(&mut blocks)
                .iter()
                .filter(|(_, b)| b.block_type() == BlockType::Stone)
                .count(),
            10
        );
    }
}
