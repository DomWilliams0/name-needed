use std::iter::once;

use common::*;
use config::WorldPreset;
use unit::world::CHUNK_SIZE;

use crate::block::BlockType;
use crate::chunk::ChunkBuilder;
use crate::loader::MemoryTerrainSource;
#[cfg(test)]
use crate::ChunkDescriptor;

pub fn from_preset(preset: WorldPreset) -> MemoryTerrainSource {
    match preset {
        WorldPreset::OneChunkWonder => one_chunk_wonder(),
        WorldPreset::MultiChunkWonder => multi_chunk_wonder(),
        WorldPreset::OneBlockWonder => one_block_wonder(),
        WorldPreset::FlatLands => flat_lands(),
        WorldPreset::Bottleneck => bottleneck(),
        WorldPreset::Stairs => stairs(),
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
            .fill_range((4, 4, -40), (9, 9, 40), |p| match p {
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
                s.set_block((half - 1, y), BlockType::Stone);
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
            s.set_block((half + 3, half), BlockType::Dirt);
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
                .fill_slice(1, BlockType::Dirt)
                .fill_slice(2, BlockType::Grass)
                .build((x, y))
        })
    });

    MemoryTerrainSource::from_chunks(chunks).expect("hardcoded world preset is wrong??!!1!")
}

/// Bottleneck for path finding
pub fn bottleneck() -> MemoryTerrainSource {
    let half_y = CHUNK_SIZE.as_i32() / 2;
    let mut rng = random::get();
    let chunks = (-2..2).map(|i| {
        let hole = rng.gen_range(1, CHUNK_SIZE.as_i32() - 1);
        ChunkBuilder::new()
            .fill_range(
                (1, 0, 0),
                (CHUNK_SIZE.as_i32() - 2, CHUNK_SIZE.as_i32() - 1, 0),
                |(x, _, _)| {
                    if x % 2 == 0 {
                        BlockType::Grass
                    } else {
                        BlockType::Dirt
                    }
                },
            )
            .fill_range((0, half_y, 1), (CHUNK_SIZE.as_i32() - 1, half_y, 4), |_| {
                BlockType::Stone
            })
            .fill_range((hole, half_y, 1), (hole + 1, half_y, 4), |_| BlockType::Air)
            // .fill_slice(-5, BlockType::Stone)
            .build((0, i))
    });

    MemoryTerrainSource::from_chunks(chunks).expect("hardcoded world preset is wrong??!!1!")
}

/// Lots of slabs
pub fn stairs() -> MemoryTerrainSource {
    let mut chunk = ChunkBuilder::new();

    const HEIGHT: i32 = 500;

    // 3x3 spiral
    const COORDS: [(i32, i32); 8] = [
        (0, 0),
        (1, 0),
        (2, 0),
        (2, 1),
        (2, 2),
        (1, 2),
        (0, 2),
        (0, 1),
    ];

    for ((x, y), z) in COORDS.iter().copied().cycle().zip(-HEIGHT..=HEIGHT) {
        let bt = if z % 2 == 0 {
            BlockType::Grass
        } else {
            BlockType::Stone
        };
        chunk = chunk.set_block((x, y, z), bt);
    }
    chunk = chunk
        .fill_slice(-HEIGHT, BlockType::Dirt)
        .fill_slice(HEIGHT, BlockType::Dirt);

    MemoryTerrainSource::from_chunks(once(chunk.build((0, 0))))
        .expect("hardcoded world preset is wrong??!!1!")
}

#[cfg(test)]
pub fn ring() -> Vec<ChunkDescriptor> {
    let fill_except_outline = |z| {
        ChunkBuilder::new().fill_range(
            (1, 1, z),
            (CHUNK_SIZE.as_i32() - 1, CHUNK_SIZE.as_i32() - 1, z),
            |_| BlockType::Stone,
        )
    };

    vec![
        // top left
        fill_except_outline(300)
            .set_block((3, 0, 300), BlockType::Grass) /* south bridge */
            .build((-1, 1)), /* NO east bridge */
        // top right
        fill_except_outline(301)
            .set_block((3, 0, 301), BlockType::Grass) /* south bridge */
            .build((0, 1)), /* NO west bridge */
        // bottom right
        fill_except_outline(300)
            .set_block((3, CHUNK_SIZE.as_i32() - 1, 300), BlockType::Grass) /* north bridge */
            .set_block((0, 3, 300), BlockType::Grass) /* west bridge */
            .build((0, 0)),
        // bottom left
        fill_except_outline(301)
            .set_block((3, CHUNK_SIZE.as_i32() - 1, 301), BlockType::Grass) /* north bridge */
            .set_block((CHUNK_SIZE.as_i32() - 1, 3, 301), BlockType::Grass) /* east bridge */
            .build((-1, 0)),
    ]
}
