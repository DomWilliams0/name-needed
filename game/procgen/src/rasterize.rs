use grid::{grid_declare, GridImpl};
use unit::world::{CHUNK_SIZE, SLAB_SIZE};

// TODO custom block types for procgen that are translated to game blocks
#[derive(Clone, Default, Debug, Copy)]
pub struct GeneratedBlock {
    // cant be zst apparently
    dummy: i32,
}

// redeclaration of slab grid
grid_declare!(pub struct SlabGrid<SlabGridImpl, GeneratedBlock>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);
