use crate::loader::terrain_source::{GeneratedSlab, TerrainSourceError};

use crate::block::Block;
use crate::chunk::slab::{Slab, SlabType};

use common::*;

use procgen::{GeneratedBlock, Planet, PlanetParamsRef};

use unit::world::{GlobalSliceIndex, SlabLocation, WorldPosition};

/// Holds lightweight arc'd and mutex'd reference to planet
#[derive(Clone)]
pub struct GeneratedTerrainSource {
    planet: Planet,
}

impl GeneratedTerrainSource {
    pub async fn new(params: PlanetParamsRef) -> BoxedResult<Self> {
        let mut planet = Planet::new(params)?;

        planet.initial_generation().await?;

        Ok(Self { planet })
    }

    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    pub async fn load_slab(&self, slab: SlabLocation) -> Result<GeneratedSlab, TerrainSourceError> {
        // TODO handle wrapping of slabs around planet boundaries
        let slab = self
            .planet
            .generate_slab(slab)
            .await
            .ok_or(TerrainSourceError::SlabOutOfBounds(slab))?;

        Ok(GeneratedSlab {
            terrain: slab.terrain.into(),
            entities: slab.entities,
        })
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
        Block::with_block_type(block.ty)
    }
}
