use std::iter::once;

use common::*;
use config::WorldPreset;
use unit::dim::CHUNK_SIZE;

use crate::block::BlockType;
use crate::chunk::ChunkBuilder;
use crate::loader::MemoryTerrainSource;

pub fn from_config() -> MemoryTerrainSource {
    match config::get().world.preset {
        WorldPreset::OneChunkWonder => one_chunk_wonder(),
        WorldPreset::MultiChunkWonder => multi_chunk_wonder(),
        WorldPreset::OneBlockWonder => one_block_wonder(),
        WorldPreset::FlatLands => flat_lands(),
        WorldPreset::PyramidMess => pyramid_mess(),
        WorldPreset::Bottleneck => bottleneck(),
    }
}

/// Multiple flat chunks with a big deep one
pub fn multi_chunk_wonder() -> MemoryTerrainSource {
    let chunks = vec![
        // 0, 0 is slice 0
        ChunkBuilder::new()
            .fill_slice(0, BlockType::Stone)
            .set_block((1, 1, 1), BlockType::Grass)
            .build((0, 0)),
        // 1, 0 is slice 1
        ChunkBuilder::new()
            .fill_slice(1, BlockType::Dirt)
            .build((1, 0)),
        // 1, 1 is slice 2
        ChunkBuilder::new()
            .fill_slice(2, BlockType::Grass)
            .build((1, 1)),
        // -1, 0 is slice 1
        ChunkBuilder::new()
            .fill_slice(1, BlockType::Dirt)
            .build((-1, 0)),
        // -1, -1 is slice 2
        ChunkBuilder::new()
            .fill_slice(2, BlockType::Grass)
            .build((-1, -1)),
        // 2, 0 is very deep
        ChunkBuilder::new()
            .fill_range((4, 4, -40), (10, 10, 40), |p| match p {
                (_, _, -40) => BlockType::Dirt,
                (_, _, 39) => BlockType::Stone,
                (_, _, z) if z < 0 => BlockType::Stone,
                _ => BlockType::Grass,
            })
            .build((2, 0)),
    ];

    MemoryTerrainSource::from_chunks(chunks.into_iter())
        .expect("hardcoded world preset is wrong??!!1!")
}

/// 1 chunk with some epic terrain
pub fn one_chunk_wonder() -> MemoryTerrainSource {
    let full = CHUNK_SIZE.as_block_coord();
    let half = full / 2;

    let chunk = ChunkBuilder::new()
        .fill_slice(0, BlockType::Stone) // fill 0 with stone
        .with_slice(1, |mut s| {
            // fill section of 1
            for x in half..full {
                for y in 0..full {
                    s.set_block((x, y), BlockType::Dirt);
                }
            }

            // step up from slice 0
            for y in 2..half - 2 {
                s.set_block((half - 1, y), BlockType::Stone)
            }
        })
        .with_slice(2, |mut s| {
            // fill smaller section of 2
            for x in half..full {
                for y in half..full {
                    s.set_block((x, y), BlockType::Grass);
                }
            }

            // step up from slice 1
            s.set_block((half + 3, half), BlockType::Dirt)
        })
        .apply(|s| {
            // stairs
            s.set_block((3, 13, 0), BlockType::Grass);
            s.set_block((4, 13, 1), BlockType::Grass);
            s.set_block((5, 13, 2), BlockType::Grass);

            // bridge
            for x in 6..13 {
                s.set_block((x, 13, 2), BlockType::Grass);
            }
        })
        .build((0, 0));

    MemoryTerrainSource::from_chunks(once(chunk)).expect("hardcoded world preset is wrong??!!1!")
}

/// A single block in a single chunk
pub fn one_block_wonder() -> MemoryTerrainSource {
    let a = ChunkBuilder::new()
        // 0, 15, 0
        .set_block((0, CHUNK_SIZE.as_i32() - 1, 0), BlockType::Stone)
        .build((0, 0));

    let b = ChunkBuilder::new()
        // -1, 16, 1, occludes 0,0,0
        .set_block((CHUNK_SIZE.as_i32() - 1, 0, 1), BlockType::Grass)
        .build((-1, 1));

    let c = ChunkBuilder::new()
        // 0, 16, 1, occludes 0,0,0
        .set_block((0, 0, 1), BlockType::Grass)
        .build((0, 1));
    let chunks = vec![a, b, c];

    MemoryTerrainSource::from_chunks(chunks.into_iter())
        .expect("hardcoded world preset is wrong??!!1!")
}

/// Multiple flat chunks at z=0
pub fn flat_lands() -> MemoryTerrainSource {
    let chunks = (-2..4).flat_map(|x| {
        (-2..2).map(move |y| {
            ChunkBuilder::new()
                .fill_slice(0, BlockType::Stone)
                .build((x, y))
        })
    });

    MemoryTerrainSource::from_chunks(chunks).expect("hardcoded world preset is wrong??!!1!")
}

/// Pyramid with some mess to test ambient occlusion across slab and chunk boundaries
pub fn pyramid_mess() -> MemoryTerrainSource {
    let chunks = vec![
        ChunkBuilder::new()
            .fill_range((0, 0, -2), (9, 9, -1), |_| BlockType::Dirt)
            .fill_range((1, 1, -1), (8, 8, 0), |_| BlockType::Stone)
            .fill_range((2, 2, 0), (7, 7, 1), |_| BlockType::Grass)
            .fill_range((3, 3, 1), (6, 6, 2), |_| BlockType::Stone)
            .fill_range((4, 4, 2), (5, 5, 3), |_| BlockType::Dirt)
            // chunk bridge
            .fill_range((0, 4, 2), (3, 5, 3), |_| BlockType::Grass)
            .set_block((0, 4, 3), BlockType::Stone)
            .build((0, 0)),
        ChunkBuilder::new()
            .fill_slice(2, BlockType::Dirt)
            .build((-1, 0)),
    ];

    MemoryTerrainSource::from_chunks(chunks.into_iter())
        .expect("hardcoded world preset is wrong??!!1!")
}

/// Bottleneck for path finding
pub fn bottleneck() -> MemoryTerrainSource {
    let half_y = CHUNK_SIZE.as_i32() / 2;
    let mut rng = thread_rng();
    let chunks = (-2..2).map(|i| {
        let hole = rng.gen_range(1, CHUNK_SIZE.as_i32() - 1);
        ChunkBuilder::new()
            .fill_range(
                (1, 0, 0),
                (CHUNK_SIZE.as_i32() - 1, CHUNK_SIZE.as_i32(), 1),
                |(x, _, _)| {
                    if x % 2 == 0 {
                        BlockType::Grass
                    } else {
                        BlockType::Dirt
                    }
                },
            )
            .fill_range((0, half_y, 1), (CHUNK_SIZE.as_i32(), half_y + 1, 5), |_| {
                BlockType::Stone
            })
            .fill_range((hole, half_y, 1), (hole + 2, half_y + 1, 5), |_| {
                BlockType::Air
            })
            .fill_slice(-5, BlockType::Stone)
            .build((0, i))
    });

    MemoryTerrainSource::from_chunks(chunks).expect("hardcoded world preset is wrong??!!1!")
}
