use crate::loader::terrain_source::TerrainSourceError;

use crate::block::Block;
use crate::chunk::slab::{Slab, SlabType};

use common::*;

use procgen::{GeneratedBlock, Planet, PlanetParams};

use unit::world::SlabLocation;

/// Holds lightweight arc'd and mutex'd reference to planet
#[derive(Clone)]
pub struct GeneratedTerrainSource {
    planet: Planet,
}

impl GeneratedTerrainSource {
    pub fn new(params: PlanetParams) -> BoxedResult<Self> {
        // TODO load a serialized planet from disk to avoid constantly regenerating
        let mut planet = Planet::new(params)?;

        info!("generating planet");
        planet.initial_generation();

        Ok(Self { planet })
    }

    pub fn planet(&self) -> &Planet {
        &self.planet
    }

    pub fn load_slab(&self, slab: SlabLocation) -> Result<Slab, TerrainSourceError> {
        let slab = self.planet.generate_slab(slab);
        Ok(slab.into())
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
        };

        Block::with_block_type(ty)
    }
}
