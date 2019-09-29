use crate::block::BlockType;
use crate::chunk::{Chunk, ChunkBuilder, CHUNK_SIZE};

pub struct World {
    chunks: Vec<Chunk>,
}

impl Default for World {
    /// 1 chunk with some epic terrain
    fn default() -> Self {
        let full: u16 = CHUNK_SIZE as u16;
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
            })
            .with_slice(2, |mut s| {
                // fill smaller section of 2
                for x in half..full {
                    for y in half..full {
                        s.set_block((x, y), BlockType::Grass);
                    }
                }
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

        Self {
            chunks: vec![chunk],
        }
    }
}

impl World {
    pub fn visible_chunks(&self) -> impl Iterator<Item = &Chunk> {
        // TODO filter visible
        self.chunks.iter()
    }
}
