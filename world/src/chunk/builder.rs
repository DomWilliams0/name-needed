use nd_iter::iter_3d;

use misc::*;
use unit::world::{BlockPosition, ChunkLocation, GlobalSliceIndex};

use crate::chunk::slab::DeepClone;
use crate::chunk::slice::SliceMut;
use crate::chunk::terrain::{RawChunkTerrain, SlabCreationPolicy};
use crate::chunk::BaseTerrain;
use crate::WorldContext;
use std::convert::TryFrom;

pub struct ChunkBuilder<C: WorldContext>(Option<RawChunkTerrain<C>>);

pub struct ChunkBuilderApply<C: WorldContext>(RawChunkTerrain<C>);

impl<C: WorldContext> ChunkBuilder<C> {
    pub fn new() -> Self {
        Self::with_terrain(RawChunkTerrain::default())
    }

    fn with_terrain(terrain: RawChunkTerrain<C>) -> Self {
        Self(Some(terrain))
    }

    fn terrain(&mut self) -> &mut RawChunkTerrain<C> {
        self.0
            .as_mut()
            .expect("builder is in an uninitialized state")
    }

    fn take_terrain(&mut self) -> RawChunkTerrain<C> {
        self.0.take().expect("builder is in an uninitialized state")
    }

    /// Panics if block position is invalid for the chunk. Will create slabs as necessary
    pub fn set_block(mut self, pos: (i32, i32, i32), block: C::BlockType) -> Self {
        self.terrain().set_block(
            BlockPosition::try_from(pos)
                .unwrap_or_else(|_| panic!("bad chunk coordinate {:?}", pos)),
            block,
            SlabCreationPolicy::CreateAll,
        );
        self
    }

    pub fn fill_slice(mut self, slice: impl Into<GlobalSliceIndex>, block: C::BlockType) -> Self {
        let do_fill = |mut slice: SliceMut<C>| slice.fill(block);
        let slice = slice.into();
        if !self
            .terrain()
            .slice_mut_with_policy(slice, SlabCreationPolicy::CreateAll, do_fill)
        {
            warn!("failed to create slice to fill"; "slice" => ?slice);
        }

        self
    }

    /// Panics if invalid range for BlockPosition
    pub fn fill_range(
        mut self,
        (fx, fy, fz): (i32, i32, i32),
        (tx, ty, tz): (i32, i32, i32),
        mut block: impl FnMut((i32, i32, i32)) -> C::BlockType,
    ) -> Self {
        for pos in iter_3d(fx..=tx, fy..=ty, fz..=tz) {
            self = self.set_block(pos, block(pos));
        }

        self
    }

    pub fn with_slice<S, F>(mut self, slice: S, mut f: F) -> Self
    where
        S: Into<GlobalSliceIndex>,
        F: FnMut(SliceMut<C>),
    {
        if let Some(slice) = self.terrain().slice_mut(slice) {
            f(slice);
        }

        self
    }

    pub fn apply<F: FnMut(&mut ChunkBuilderApply<C>)>(mut self, mut f: F) -> Self {
        // steal terrain out of self
        let terrain = self.take_terrain();
        let mut apply = ChunkBuilderApply(terrain);

        f(&mut apply);

        // steal back from apply
        Self::with_terrain(apply.0)
    }

    pub fn build<P: Into<ChunkLocation>>(self, pos: P) -> ChunkDescriptor<C> {
        ChunkDescriptor {
            terrain: self.into_inner(),
            chunk_pos: pos.into(),
        }
    }

    pub fn into_inner(mut self) -> RawChunkTerrain<C> {
        self.take_terrain()
    }
}

impl<C: WorldContext> ChunkBuilderApply<C> {
    /// Needs to take self by reference instead of value like ChunkBuilder, so can't simply
    /// use DerefMut
    pub fn set_block(&mut self, pos: (i32, i32, i32), block: C::BlockType) -> &mut Self {
        self.0.set_block(
            BlockPosition::try_from(pos)
                .unwrap_or_else(|_| panic!("bad chunk coordinate {:?}", pos)),
            block,
            SlabCreationPolicy::CreateAll,
        );
        self
    }
}

impl<C: WorldContext> Default for ChunkBuilder<C> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ChunkDescriptor<C: WorldContext> {
    pub terrain: RawChunkTerrain<C>,
    pub chunk_pos: ChunkLocation,
}

impl<C: WorldContext> From<ChunkDescriptor<C>> for (ChunkLocation, RawChunkTerrain<C>) {
    fn from(desc: ChunkDescriptor<C>) -> Self {
        (desc.chunk_pos, desc.terrain)
    }
}

impl<C: WorldContext> DeepClone for ChunkDescriptor<C> {
    fn deep_clone(&self) -> Self {
        Self {
            chunk_pos: self.chunk_pos,
            terrain: self.terrain.deep_clone(),
        }
    }
}

impl<C: WorldContext> DeepClone for ChunkBuilder<C> {
    fn deep_clone(&self) -> Self {
        ChunkBuilder(self.0.as_ref().map(|t| t.deep_clone()))
    }
}

#[cfg(test)]
mod tests {
    use unit::world::GlobalSliceIndex;

    use crate::chunk::{BaseTerrain, ChunkBuilder};
    use crate::helpers::{DummyBlockType, DummyWorldContext};

    #[test]
    fn fill_slice() {
        // check that filling a slice with a block really does
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .fill_slice(0, DummyBlockType::Grass)
            .into_inner();

        assert!(c
            .slice(GlobalSliceIndex::new(0))
            .unwrap()
            .all_blocks_are(DummyBlockType::Grass));
        assert!(c
            .slice(GlobalSliceIndex::new(1))
            .unwrap()
            .all_blocks_are(DummyBlockType::Air));
    }

    #[test]
    fn set_block() {
        // check setting a specific block works
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .set_block((2, 2, 1), DummyBlockType::Stone)
            .set_block((3, 3, 3), DummyBlockType::Grass)
            .into_inner();

        assert_eq!(
            c.get_block_tup((2, 2, 1)).unwrap().block_type(),
            DummyBlockType::Stone
        );

        assert_eq!(
            c.get_block_tup((3, 3, 3)).unwrap().block_type(),
            DummyBlockType::Grass
        );
    }

    #[test]
    fn apply() {
        // check that the apply helper works as intended
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .apply(|c| {
                c.set_block((1, 1, 1), DummyBlockType::Grass);
                c.set_block((1, 2, 1), DummyBlockType::Grass);
            })
            .into_inner();

        assert_eq!(
            c.get_block_tup((1, 1, 1)).map(|b| b.block_type()),
            Some(DummyBlockType::Grass)
        );
        assert_eq!(
            c.get_block_tup((1, 2, 1)).map(|b| b.block_type()),
            Some(DummyBlockType::Grass)
        );
    }

    #[test]
    fn fill_range() {
        // check that range filling works as intended
        let c = ChunkBuilder::<DummyWorldContext>::new()
            .fill_range((0, 0, 0), (2, 2, 2), |_| DummyBlockType::Stone)
            .into_inner();
        let mut blocks = Vec::new();

        // expected to have filled 0-2 on all 3 dimensions
        assert_eq!(
            c.blocks(&mut blocks)
                .iter()
                .filter(|(_, b)| b.block_type() == DummyBlockType::Stone)
                .count(),
            3 * 3 * 3
        );

        let c = ChunkBuilder::<DummyWorldContext>::new()
            .fill_range((0, 0, 0), (9, 0, 0), |_| DummyBlockType::Stone)
            .into_inner();
        assert_eq!(
            c.blocks(&mut blocks)
                .iter()
                .filter(|(_, b)| b.block_type() == DummyBlockType::Stone)
                .count(),
            10
        );
    }
}
