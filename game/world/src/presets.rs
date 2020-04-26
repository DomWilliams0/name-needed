use config::WorldPreset;

use crate::block::{BlockHeight, BlockType};
use crate::chunk::CHUNK_SIZE;
use crate::{ChunkBuilder, World};
use common::*;

pub fn from_config() -> World {
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
pub fn multi_chunk_wonder() -> World {
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

    World::from_chunks(chunks)
}

/// 1 chunk with some epic terrain
pub fn one_chunk_wonder() -> World {
    let full: u16 = CHUNK_SIZE.as_u16();
    let half: u16 = full / 2;

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
                s.set_block((half - 1, y), (BlockType::Stone, BlockHeight::Half))
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
            s.set_block((half + 3, half), (BlockType::Dirt, BlockHeight::Half))
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

    World::from_chunks(vec![chunk])
}

/// A single block in a single chunk
pub fn one_block_wonder() -> World {
    let chunk = ChunkBuilder::new()
        .set_block((2, 2, 2), BlockType::Grass)
        .set_block((1, 1, 1), BlockType::Stone)
        .set_block((2, 1, 1), BlockType::Stone)
        .set_block((3, 1, 1), BlockType::Dirt)
        .set_block((1, 2, 1), BlockType::Stone)
        .set_block((3, 2, 1), BlockType::Stone)
        .set_block((1, 3, 1), BlockType::Dirt)
        .set_block((2, 3, 1), BlockType::Stone)
        .set_block((3, 3, 1), BlockType::Stone)
        .build((0, 0));

    World::from_chunks(vec![chunk])
}

/// Multiple flat chunks at z=0
pub fn flat_lands() -> World {
    let chunks = (-2..4)
        .flat_map(|x| {
            (-2..2).map(move |y| {
                ChunkBuilder::new()
                    .fill_slice(0, BlockType::Stone)
                    .build((x, y))
            })
        })
        .collect_vec();

    World::from_chunks(chunks)
}

/// Pyramid with some mess to test ambient occlusion across slab and chunk boundaries
pub fn pyramid_mess() -> World {
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

    World::from_chunks(chunks)
}

/// Bottleneck for path finding
pub fn bottleneck() -> World {
    let half_y = CHUNK_SIZE.as_i32() / 2;
    let mut rng = thread_rng();
    let chunks = (-2..2)
        .map(|i| {
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
        })
        .collect_vec();

    World::from_chunks(chunks)
}
