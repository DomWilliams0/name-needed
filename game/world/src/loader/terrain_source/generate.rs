use crate::block::BlockType;
use crate::chunk::ChunkBuilder;
use crate::chunk::RawChunkTerrain;
use crate::loader::terrain_source::{PreprocessedTerrain, TerrainSourceError};
use crate::loader::TerrainSource;
use common::*;
use unit::dim::CHUNK_SIZE;
use unit::world::ChunkPosition;

pub struct GeneratedTerrainSource {
    bounds: (ChunkPosition, ChunkPosition),
    seed: u64,
    height_scale: f64,
}

impl GeneratedTerrainSource {
    pub fn new(seed: Option<u64>, radius: u32, height_scale: f64) -> Result<Self, &'static str> {
        if radius < 1 {
            return Err("radius should be >0");
        }

        let seed = if let Some(seed) = seed {
            debug!("using specified seed for terrain generation"; "seed" => seed);
            seed
        } else {
            let seed = thread_rng().gen();
            debug!("using random seed for terrain generation"; "seed" => seed);
            seed
        };

        // radius is excluding 0,0
        let radius = radius as i32;
        let bounds = (
            ChunkPosition(-radius, -radius),
            ChunkPosition(radius, radius),
        );

        Ok(Self {
            seed,
            bounds,
            height_scale,
        })
    }
}

impl TerrainSource for GeneratedTerrainSource {
    fn world_bounds(&self) -> &(ChunkPosition, ChunkPosition) {
        &self.bounds
    }

    fn all_chunks(&mut self) -> Vec<ChunkPosition> {
        let radius = (self.bounds.1).0 as u16;
        spiral::ChebyshevIterator::new(0, 0, radius)
            .map(|(x, y)| ChunkPosition(x, y))
            .collect()
    }

    fn preprocess(
        &self,
        chunk: ChunkPosition,
    ) -> Box<dyn FnOnce() -> Result<Box<dyn PreprocessedTerrain>, TerrainSourceError>> {
        let seed = self.seed;
        let height_scale = self.height_scale;
        Box::new(move || {
            let chunk = chunk.into();
            let terrain_desc = procgen::generate_chunk(chunk, CHUNK_SIZE.as_usize(), seed, 30.0);

            // height map -> raw chunk terrain
            let mut terrain = ChunkBuilder::new();
            for ((y, x), height) in (0..CHUNK_SIZE.as_i32())
                .cartesian_product(0..CHUNK_SIZE.as_i32())
                .zip(terrain_desc.heightmap.into_iter())
            {
                let mul = height * height_scale;
                let ground = mul as i32;
                let block_type = if mul.fract() < 0.2 {
                    BlockType::LightGrass
                } else {
                    BlockType::Grass
                };
                terrain = terrain.fill_range((x, y, 0), (x, y, ground), |(_, _, z)| {
                    if z < ground {
                        BlockType::Stone
                    } else {
                        block_type
                    }
                });
            }

            Ok(Box::new(terrain.into_inner()))
        })
    }

    fn load_chunk(
        &mut self,
        _: ChunkPosition,
        preprocessed: Box<dyn PreprocessedTerrain>,
    ) -> Result<RawChunkTerrain, TerrainSourceError> {
        Ok(preprocessed.into_raw_terrain())
    }
}

impl PreprocessedTerrain for RawChunkTerrain {
    fn into_raw_terrain(self: Box<Self>) -> RawChunkTerrain {
        *self
    }
}
