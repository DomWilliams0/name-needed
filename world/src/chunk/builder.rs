use nd_iter::iter_3d;
use std::collections::HashMap;

use misc::*;
use unit::world::{
    BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SliceIndex,
    WorldPosition, WorldPositionRange,
};

use crate::chunk::slab::DeepClone;
use crate::chunk::slice::SliceMut;
use crate::chunk::terrain::{SlabCreationPolicy, SlabStorage};
use crate::loader::split_range_across_slabs;
use crate::WorldContext;
use std::convert::TryFrom;

pub struct ChunkBuilder<C: WorldContext> {
    inner: Option<SlabStorage<C>>,
    cowed_range: (SlabIndex, SlabIndex),
}

pub struct WorldBuilder<C: WorldContext>(HashMap<ChunkLocation, ChunkBuilder<C>>);

pub struct ChunkBuilderApply<C: WorldContext>(SlabStorage<C>);

impl<C: WorldContext> ChunkBuilder<C> {
    pub fn new() -> Self {
        Self::with_terrain(SlabStorage::default())
    }

    pub fn empty() -> Self {
        Self {
            inner: None,
            cowed_range: (SlabIndex::MAX, SlabIndex::MIN),
        }
    }

    fn with_terrain(terrain: SlabStorage<C>) -> Self {
        Self {
            inner: Some(terrain),
            cowed_range: (SlabIndex::MAX, SlabIndex::MIN),
        }
    }

    fn terrain(&mut self) -> &mut SlabStorage<C> {
        let storage = self
            .inner
            .as_mut()
            .expect("builder is in an uninitialized state");

        let range = storage.slab_range();
        if range != self.cowed_range {
            for idx in range.0 .0..=range.1 .0 {
                storage
                    .slab_data_mut(SlabIndex(idx))
                    .unwrap()
                    .terrain
                    .cow_clone();
            }
            self.cowed_range = range;
        }

        storage
    }

    fn take_terrain(&mut self) -> SlabStorage<C> {
        self.inner
            .take()
            .expect("builder is in an uninitialized state")
    }

    /// Panics if block position is invalid for the chunk. Will create slabs as necessary
    pub fn set_block(mut self, pos: (i32, i32, i32), block: C::BlockType) -> Self {
        self.terrain().set_block(
            BlockPosition::try_from(pos)
                .unwrap_or_else(|_| panic!("bad chunk coordinate {:?}", pos)),
            block,
            SlabCreationPolicy::CreateAll {
                placeholders: false,
            },
        );
        self
    }

    pub fn fill_slice(mut self, slice: impl Into<GlobalSliceIndex>, block: C::BlockType) -> Self {
        let do_fill = |mut slice: SliceMut<C>| slice.fill(block);
        let slice = slice.into();
        if !self.terrain().slice_mut_with_policy(
            slice,
            SlabCreationPolicy::CreateAll {
                placeholders: false,
            },
            do_fill,
        ) {
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

    pub fn into_inner(mut self) -> SlabStorage<C> {
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
            SlabCreationPolicy::CreateAll {
                placeholders: false,
            },
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
    pub terrain: SlabStorage<C>,
    pub chunk_pos: ChunkLocation,
}

impl<C: WorldContext> From<ChunkDescriptor<C>> for (ChunkLocation, SlabStorage<C>) {
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
        ChunkBuilder {
            inner: self.inner.as_ref().map(|t| t.deep_clone()),
            cowed_range: self.cowed_range,
        }
    }
}

impl<C: WorldContext> WorldBuilder<C> {
    pub fn new() -> Self {
        Self(Default::default())
    }
    pub fn set(mut self, pos: (i32, i32, i32), b: C::BlockType) -> Self {
        let world_pos = WorldPosition::from(pos);
        let chunk = ChunkLocation::from(world_pos);
        let c = self.0.entry(chunk).or_insert_with(|| ChunkBuilder::new());

        let tmp = std::mem::replace(c, ChunkBuilder::empty());
        let actual_pos = BlockPosition::from(world_pos);
        *c = tmp.set_block(
            (
                actual_pos.x() as i32,
                actual_pos.y() as i32,
                actual_pos.z().slice(),
            ),
            b,
        ); // wtf
        self
    }

    pub fn fill_range(
        mut self,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        b: C::BlockType,
    ) -> Self {
        let range = WorldPositionRange::with_inclusive_range(from, to);
        for (slab, update) in split_range_across_slabs::<C>(range, b) {
            let chunk = slab.chunk;
            let c = self.0.entry(chunk).or_insert_with(|| ChunkBuilder::new());

            let tmp = std::mem::replace(c, ChunkBuilder::empty());
            let (xs, ys, zs) = update.0.ranges();

            let zs = (
                LocalSliceIndex::new_unchecked(zs.0).to_global(slab.slab),
                LocalSliceIndex::new_unchecked(zs.1).to_global(slab.slab),
            );

            *c = tmp.fill_range(
                (xs.0 as i32, ys.0 as i32, zs.0.slice()),
                (xs.1 as i32, ys.1 as i32, zs.1.slice()),
                |_| b,
            );
        }

        self
    }

    pub fn build(self) -> impl Iterator<Item = ChunkDescriptor<C>> {
        self.0.into_iter().map(|(loc, c)| c.build(loc))
    }
}

#[cfg(test)]
mod tests {
    use unit::world::GlobalSliceIndex;

    use crate::chunk::ChunkBuilder;
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
