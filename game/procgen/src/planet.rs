use common::*;
use grid::{grid_declare, GridImpl};
use unit::world::{ChunkLocation, SlabLocation};
use unit::world::{CHUNK_SIZE, SLAB_SIZE};

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet {
    radius: u32,
}

pub struct PlanetParams {
    pub seed: u64,
    pub radius: u32,
    // TODO square with width/height in some unit or circle?
}

// TODO custom block types for procgen that are translated to game blocks
#[derive(Clone, Default, Debug, Copy)]
pub struct GeneratedBlock {
    // cant be zst apparently
    dummy: i32,
}

// redeclaration of slab grid
// TODO move this
grid_declare!(pub struct SlabGrid<SlabGridImpl, GeneratedBlock>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

impl Planet {
    // TODO actual error type
    pub fn new(params: PlanetParams) -> Result<Planet, &'static str> {
        Ok(Self {
            radius: params.radius,
        })

        // TODO begin actual generation? or up to the caller for async goodness
    }

    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        // radius is excluding 0,0
        let radius = self.radius as i32;
        (
            ChunkLocation(-radius, -radius),
            ChunkLocation(radius, radius),
        )
    }

    pub fn generate_slab(&self, slab: SlabLocation) -> SlabGrid {
        SlabGrid::default()
    }
}
