use crate::block::{Block, BlockDurability, BlockOpacity};
pub use crate::chunk::slice::SLICE_SIZE;
use crate::loader::{GeneratedSlab, WorldTerrainUpdate};
use async_trait::async_trait;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use common::Derivative;
use unit::world::{ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition};

pub trait WorldContext: 'static + Send + Sync + Sized {
    type AssociatedBlockData;
    type BlockType: BlockType;

    type GeneratedTerrainSource: GeneratedTerrainSource<Self> + Send + Sync;
    type GeneratedBlockDetails:  Send + Sync;
    type GeneratedEntityDesc:  Send + Sync;

    fn air_slice() -> &'static [Block<Self>; SLICE_SIZE];

    // non-air, solid, walkable blocks to use in presets
    const PRESET_TYPES: [Self::BlockType; 3];
}

/// Panics in all methods
#[derive(Derivative)]
#[derivative(Clone(bound=""))]
pub struct NopGeneratedTerrainSource<C: WorldContext>(PhantomData<C>);

pub trait BlockType: Copy + Debug + Eq + Hash + Sync + Send {
    const AIR: Self;

    fn opacity(&self) -> BlockOpacity;
    fn durability(&self) -> BlockDurability;

    fn is_air(&self) -> bool;
    /// TODO very temporary "walkability" for block types
    fn can_be_walked_on(&self) -> bool;

    fn render_color(&self) -> color::Color;
}

#[async_trait]
pub trait GeneratedTerrainSource<C: WorldContext>: Clone {
    async fn prepare_for_chunks(&self, range: (ChunkLocation, ChunkLocation));
    async fn query_block(&self, block: WorldPosition) -> Option<C::GeneratedBlockDetails>;

    /// For debug rendering only
    async fn feature_boundaries_in_range(
        &self,
        chunks: &[ChunkLocation],
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        output: &mut Vec<(usize, WorldPosition)>,
    );

    async fn steal_queued_block_updates(&self, out: &mut HashSet<WorldTerrainUpdate<C>>);

    async fn generate_slab(&self, slab: SlabLocation) -> Option<GeneratedSlab<C>>;
    async fn find_ground_level(&self, block: WorldPosition) -> Option<GlobalSliceIndex>;
}

// lol
#[async_trait]
impl <C: WorldContext>GeneratedTerrainSource<C> for NopGeneratedTerrainSource<C> {
    async fn prepare_for_chunks(&self, _: (ChunkLocation, ChunkLocation)) {
        unreachable!()
    }

    async fn query_block(&self, _: WorldPosition) -> Option<C::GeneratedBlockDetails> {
        unreachable!()
    }

    async fn feature_boundaries_in_range(&self, _: &[ChunkLocation], _: (GlobalSliceIndex, GlobalSliceIndex), _: &mut Vec<(usize, WorldPosition)>) {
        unreachable!()
    }

    async fn steal_queued_block_updates(&self, _: &mut HashSet<WorldTerrainUpdate<C>>) {
        unreachable!()
    }

    async fn generate_slab(&self, _: SlabLocation) -> Option<GeneratedSlab<C>> {
        unreachable!()
    }

    async fn find_ground_level(&self, _: WorldPosition) -> Option<GlobalSliceIndex> {
        unreachable!()
    }
}
