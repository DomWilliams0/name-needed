use nd_iter::iter_3d;

use common::*;
use unit::world::{BlockPosition, ChunkLocation, GlobalSliceIndex};

use crate::block::Block;
use crate::chunk::slab::DeepClone;
use crate::chunk::slice::SliceMut;
use crate::chunk::terrain::{RawChunkTerrain, SlabCreationPolicy};
use crate::chunk::BaseTerrain;

pub struct ChunkBuilder(Option<RawChunkTerrain>);

pub struct ChunkBuilderApply(RawChunkTerrain);

impl ChunkBuilder {
    pub fn new() -> Self {
        Self::with_terrain(RawChunkTerrain::default())
    }

    fn with_terrain(terrain: RawChunkTerrain) -> Self {
        Self(Some(terrain))
    }

    fn terrain(&mut self) -> &mut RawChunkTerrain {
        self.0
            .as_mut()
            .expect("builder is in an uninitialized state")
    }

    fn take_terrain(&mut self) -> RawChunkTerrain {
        self.0.take().expect("builder is in an uninitialized state")
    }

    /// Will create slabs as necessary
    pub fn set_block<P, B>(mut self, pos: P, block: B) -> Self
    where
        P: Into<BlockPosition>,
        B: Into<Block>,
    {
        self.terrain()
            .set_block(pos, block, SlabCreationPolicy::CreateAll);
        self
    }

    pub fn fill_slice<S, B>(mut self, slice: S, block: B) -> Self
    where
        S: Into<GlobalSliceIndex>,
        B: Into<Block>,
    {
        let do_fill = |mut slice: SliceMut| slice.fill(block);
        let slice = slice.into();
        if !self
            .terrain()
            .slice_mut_with_policy(slice, SlabCreationPolicy::CreateAll, do_fill)
        {
            warn!("failed to create slice to fill"; "slice" => ?slice);
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

        for pos in iter_3d(fx..=tx, fy..=ty, fz..=tz) {
            self = self.set_block(pos, block(pos));
        }

        self
    }

    pub fn with_slice<S, F>(mut self, slice: S, mut f: F) -> Self
    where
        S: Into<GlobalSliceIndex>,
        F: FnMut(SliceMut),
    {
        if let Some(slice) = self.terrain().slice_mut(slice) {
            f(slice);
        }

        self
    }

    pub fn apply<F: FnMut(&mut ChunkBuilderApply)>(mut self, mut f: F) -> Self {
        // steal terrain out of self
        let terrain = self.take_terrain();
        let mut apply = ChunkBuilderApply(terrain);

        f(&mut apply);

        // steal back from apply
        Self::with_terrain(apply.0)
    }

    pub fn build<P: Into<ChunkLocation>>(self, pos: P) -> ChunkDescriptor {
        ChunkDescriptor {
            terrain: self.into_inner(),
            chunk_pos: pos.into(),
        }
    }

    pub fn into_inner(mut self) -> RawChunkTerrain {
        self.take_terrain()
    }
}

impl ChunkBuilderApply {
    /// Needs to take self by reference instead of value like ChunkBuilder, so can't simply
    /// use DerefMut
    pub fn set_block<P, B>(&mut self, pos: P, block: B) -> &mut Self
    where
        P: Into<BlockPosition>,
        B: Into<Block>,
    {
        self.0.set_block(pos, block, SlabCreationPolicy::CreateAll);
        self
    }
}

impl Default for ChunkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ChunkDescriptor {
    pub terrain: RawChunkTerrain,
    pub chunk_pos: ChunkLocation,
}

impl From<ChunkDescriptor> for (ChunkLocation, RawChunkTerrain) {
    fn from(desc: ChunkDescriptor) -> Self {
        (desc.chunk_pos, desc.terrain)
    }
}

impl DeepClone for ChunkDescriptor {
    fn deep_clone(&self) -> Self {
        Self {
            chunk_pos: self.chunk_pos,
            terrain: self.terrain.deep_clone(),
        }
    }
}

impl DeepClone for ChunkBuilder {
    fn deep_clone(&self) -> Self {
        ChunkBuilder(self.0.as_ref().map(|t| t.deep_clone()))
    }
}

#[cfg(test)]
mod tests {
    use unit::world::GlobalSliceIndex;

    use crate::block::BlockType;
    use crate::chunk::{BaseTerrain, ChunkBuilder};

    #[test]
    fn fill_slice() {
        // check that filling a slice with a block really does
        let c = ChunkBuilder::new()
            .fill_slice(0, BlockType::Grass)
            .into_inner();

        assert!(c
            .slice(GlobalSliceIndex::new(0))
            .unwrap()
            .all_blocks_are(BlockType::Grass));
        assert!(c
            .slice(GlobalSliceIndex::new(1))
            .unwrap()
            .all_blocks_are(BlockType::Air));
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
            .fill_range((0, 0, 0), (2, 2, 2), |_| BlockType::Stone)
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

        let c = ChunkBuilder::new()
            .fill_range((0, 0, 0), (9, 0, 0), |_| BlockType::Stone)
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
