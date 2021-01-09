use crate::loader::terrain_source::{PreprocessedTerrain, TerrainSourceError};
use crate::loader::TerrainSource;

use crate::block::{Block, BlockType};
use crate::chunk::slab::{Slab, SlabGrid, SlabType};
use common::*;
use grid::GridImpl;
use procgen::{Planet, PlanetParams};
use unit::world::{ChunkLocation, SlabIndex, SlabLocation};

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
}

impl TerrainSource for GeneratedTerrainSource {
    fn world_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        self.planet.chunk_bounds()
    }

    fn preprocess(
        &self,
        slab: SlabLocation,
    ) -> Box<dyn FnOnce() -> Result<Box<dyn PreprocessedTerrain>, TerrainSourceError>> {
        let planet = self.planet.clone();
        Box::new(move || {
            let procgen_slab = planet.generate_slab(slab);
            let slab_grid = convert_grid(procgen_slab);
            // TODO might be able to use SlabGridImpl here and avoid double boxing
            Ok(Box::new(Slab::from_grid(slab_grid, SlabType::Normal)))
        })
    }

    fn load_slab(
        &mut self,
        _: SlabLocation,
        preprocess_result: Box<dyn PreprocessedTerrain>,
    ) -> Result<Slab, TerrainSourceError> {
        Ok(preprocess_result.into_slab())
    }

    fn prepare_for_chunks(&mut self, range: (ChunkLocation, ChunkLocation)) {
        self.planet.prepare_for_chunks(range);
    }
}

fn convert_grid(generated: procgen::SlabGrid) -> SlabGrid {
    assert_eq!(generated.array().len(), SlabGrid::FULL_SIZE);

    let mut slab = SlabGrid::default();

    // TODO populate slab grid from generated
    slab[&[0, 0, 0]] = Block::with_block_type(BlockType::Grass);

    slab
}

impl PreprocessedTerrain for Slab {
    fn into_slab(self: Box<Self>) -> Slab {
        *self
    }
}
