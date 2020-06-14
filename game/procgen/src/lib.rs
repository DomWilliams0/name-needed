use common::*;
use noise::{Fbm, MultiFractal, NoiseFn, Seedable};

/// Not block for block, just a high level description of the terrain that can be converted to
/// `RawChunkTerrain` back in `world`
pub struct TerrainDescription {
    /// chunk_size x chunk_size
    pub heightmap: Vec<f64>,
}

/// https://rosettacode.org/wiki/Map_range#Rust
fn map_range(from_range: (f64, f64), to_range: (f64, f64), s: f64) -> f64 {
    to_range.0 + (s - from_range.0) * (to_range.1 - to_range.0) / (from_range.1 - from_range.0)
}

pub fn generate_chunk(
    chunk_position: (i32, i32),
    chunk_size: usize,
    seed: u64,
    noise_scale: f64,
) -> TerrainDescription {
    let mut heightmap = Vec::with_capacity(chunk_size * chunk_size);
    heightmap.resize(chunk_size * chunk_size, 0.0);
    let heightmap_slice = &mut heightmap[..chunk_size * chunk_size];

    let noise = Fbm::new()
        .set_seed(seed as u32)
        .set_octaves(3)
        .set_lacunarity(0.8)
        .set_persistence(0.65);

    let chunk_size = chunk_size as i32;

    // TODO generate lower res noise and scale up
    let mut i = 0;
    (0..chunk_size)
        .cartesian_product(0..chunk_size)
        .for_each(|(y, x)| {
            let nx = ((chunk_position.0 * chunk_size) + x) as f64;
            let ny = ((chunk_position.1 * chunk_size) + y) as f64;
            let val = noise.get([nx / noise_scale, ny / noise_scale, 0.4]);

            let height = map_range((-1.0, 1.0), (0.0, 1.0), val);
            heightmap_slice[i] = height;
            i += 1;
        });

    debug_assert_eq!(heightmap.len() as i32, chunk_size * chunk_size);
    TerrainDescription { heightmap }
}
