use crate::loader::terrain_source::TerrainSourceError;

use crate::block::Block;
use crate::chunk::slab::{Slab, SlabType};

use common::*;

use procgen::{GeneratedBlock, Planet, PlanetParams};

use unit::world::{GlobalSliceIndex, SlabLocation, WorldPosition};

/// Holds lightweight arc'd and mutex'd reference to planet
#[derive(Clone)]
pub struct GeneratedTerrainSource {
    planet: Planet,
}

impl GeneratedTerrainSource {
    pub async fn new(params: PlanetParams) -> BoxedResult<Self> {
        let mut planet = Planet::new(params)?;

        planet.initial_generation().await?;

        Ok(Self { planet })
    }

    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    pub async fn load_slab(&self, slab: SlabLocation) -> Result<Slab, TerrainSourceError> {
        // TODO handle wrapping of slabs around planet boundaries
        let slab = self
            .planet
            .generate_slab(slab)
            .await
            .ok_or(TerrainSourceError::SlabOutOfBounds(slab))?;
        Ok(slab.into())
    }

    pub async fn get_ground_level(&self, block: WorldPosition) -> Option<GlobalSliceIndex> {
        self.planet.find_ground_level(block).await
    }
}

impl From<procgen::SlabGrid> for Slab {
    fn from(grid: procgen::SlabGrid) -> Self {
        Slab::from_other_grid(grid, SlabType::Normal)
    }
}

impl From<&procgen::GeneratedBlock> for Block {
    fn from(block: &GeneratedBlock) -> Self {
        use crate::block::BlockType as B;
        use procgen::BlockType as A;
        let ty = match block.ty {
            A::Air => B::Air,
            A::Stone => B::Stone,
            A::Dirt => B::Dirt,
            A::Grass => B::Grass,
            A::Sand => B::Sand,
            A::SolidWater => B::SolidWater,
        };

        Block::with_block_type(ty)
    }
}
