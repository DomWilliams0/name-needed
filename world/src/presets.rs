use crate::block::{BlockHeight, BlockType};
use crate::chunk::CHUNK_SIZE;
use crate::{ChunkBuilder, World};

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
            for y in 2..half-2 {
                s.set_block((half-1,y), (BlockType::Stone, BlockHeight::Half))
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
        .set_block((1, 1, 1), BlockType::Stone)
        .build((0, 0));

    World::from_chunks(vec![chunk])
}
