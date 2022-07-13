use crate::AssociatedBlockData;
use async_trait::async_trait;
use std::collections::HashSet;
use unit::world::{
    ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition, WorldPositionRange,
};
use world::block::Block;
use world::loader::{GeneratedSlab, TerrainSource, WorldTerrainUpdate};
use world::{Slab, SlabType};

pub struct WorldContext;

// TODO move this out of this file
const AIR_SLICE: [Block<WorldContext>; world::SLICE_SIZE] = [Block::air(); world::SLICE_SIZE];

impl world::WorldContext for WorldContext {
    type AssociatedBlockData = AssociatedBlockData;
    type BlockType = world_types::BlockType;

    #[cfg(feature = "worldprocgen")]
    type GeneratedTerrainSource = PlanetTerrainSource;
    #[cfg(not(feature = "worldprocgen"))]
    type GeneratedTerrainSource = world::NopGeneratedTerrainSource<Self>;

    #[cfg(feature = "worldprocgen")]
    type GeneratedBlockDetails = procgen::BlockQueryResult;
    #[cfg(not(feature = "worldprocgen"))]
    type GeneratedBlockDetails = ();

    type GeneratedEntityDesc = world_types::EntityDescription;

    fn air_slice() -> &'static [Block<Self>; world::SLICE_SIZE] {
        &AIR_SLICE
    }

    const PRESET_TYPES: [Self::BlockType; 3] = [
        world_types::BlockType::Stone,
        world_types::BlockType::Dirt,
        world_types::BlockType::Grass,
    ];
}

#[derive(Clone)]
#[cfg(feature = "worldprocgen")]
pub struct PlanetTerrainSource(pub procgen::Planet);

#[cfg(feature = "worldprocgen")]
impl From<PlanetTerrainSource> for TerrainSource<WorldContext> {
    fn from(src: PlanetTerrainSource) -> Self {
        TerrainSource::Generated(src)
    }
}

#[cfg(feature = "worldprocgen")]
#[async_trait]
impl world::GeneratedTerrainSource<WorldContext> for PlanetTerrainSource {
    async fn prepare_for_chunks(&self, range: (ChunkLocation, ChunkLocation)) {
        self.0.prepare_for_chunks(range).await
    }

    async fn query_block(&self, block: WorldPosition) -> Option<procgen::BlockQueryResult> {
        self.0.query_block(block).await
    }

    async fn feature_boundaries_in_range(
        &self,
        chunks: &[ChunkLocation],
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        output: &mut Vec<(usize, WorldPosition)>,
    ) {
        self.0
            .feature_boundaries_in_range(chunks.iter().copied(), z_range, |feat, point| {
                output.push((feat, point))
            })
            .await
    }

    async fn steal_queued_block_updates(
        &self,
        out: &mut HashSet<WorldTerrainUpdate<WorldContext>>,
    ) {
        self.0
            .steal_world_updates(|updates| {
                out.extend(updates.map(|(pos, block)| {
                    WorldTerrainUpdate::new(WorldPositionRange::with_single(pos), block.ty)
                }))
            })
            .await
    }

    async fn generate_slab(&self, slab: SlabLocation) -> Option<GeneratedSlab<WorldContext>> {
        self.0.generate_slab(slab).await.map(|slab| GeneratedSlab {
            terrain: Slab::from_other_grid(slab.terrain, SlabType::Normal, |b| {
                Block::with_block_type(b.ty)
            }),
            entities: slab.entities,
        })
    }

    async fn find_ground_level(&self, block: WorldPosition) -> Option<GlobalSliceIndex> {
        self.0.find_ground_level(block).await
    }
}
