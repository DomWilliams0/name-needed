use crate::block::{Block, BlockDurability, BlockOpacity};
use crate::chunk::slab::SlabGridImpl;
pub use crate::chunk::slice::SLICE_SIZE;
use crate::loader::{GeneratedSlab, WorldTerrainUpdate};
use async_trait::async_trait;
use futures::future::BoxFuture;
use misc::Derivative;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;
use unit::world::{ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPoint, WorldPosition};

pub trait WorldContext: 'static + Send + Sync + Sized {
    type BlockType: BlockType;

    type GeneratedTerrainSource: GeneratedTerrainSource<Self> + Send + Sync;
    type GeneratedBlockDetails: Send + Sync;
    type GeneratedEntityDesc: Send + Sync;

    fn air_slice() -> &'static [Block<Self>; SLICE_SIZE];

    fn all_air() -> Arc<SlabGridImpl<Self>>;
    fn all_stone() -> Arc<SlabGridImpl<Self>>;

    // non-air, solid, walkable blocks to use in presets
    const PRESET_TYPES: [Self::BlockType; 3];

    fn slab_grid_of_all(bt: Self::BlockType) -> Arc<SlabGridImpl<Self>> {
        let mut g = crate::SlabGrid::from_iter(std::iter::repeat(Block::with_block_type(bt)));
        let boxed = g.into_boxed_impl();
        Arc::from(boxed)
    }

    type SearchToken: SearchToken;
}

pub trait SearchToken: Debug + Clone + Send + Sync {
    fn get_updated_search_source(&self) -> BoxFuture<UpdatedSearchSource>;
}

pub enum UpdatedSearchSource {
    Unchanged,
    New(WorldPoint),
    Abort,
}

/// Panics in all methods
#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
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
impl<C: WorldContext> GeneratedTerrainSource<C> for NopGeneratedTerrainSource<C> {
    async fn prepare_for_chunks(&self, _: (ChunkLocation, ChunkLocation)) {
        unreachable!()
    }

    async fn query_block(&self, _: WorldPosition) -> Option<C::GeneratedBlockDetails> {
        unreachable!()
    }

    async fn feature_boundaries_in_range(
        &self,
        _: &[ChunkLocation],
        _: (GlobalSliceIndex, GlobalSliceIndex),
        _: &mut Vec<(usize, WorldPosition)>,
    ) {
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
