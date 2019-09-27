use crate::block::BlockType;
use crate::chunk;
use crate::chunk::Chunk;

pub struct World {
    chunks: Vec<Chunk>,
}

impl Default for World {
    /// 1 chunk with a few pillars
    fn default() -> Self {
        let chunk = {
            let mut c = Chunk::empty((0, 0));

            // odd staircase
            for i in 0u16..chunk::CHUNK_SIZE as u16 {
                c.set_block((0, i, i32::from(i)), BlockType::Hi);
            }

            // fill 0
            for b in c.slice_mut(0).iter_mut() {
                *b = BlockType::Dirt;
            }

            c.set_block((0, 0, 0), BlockType::Hi);
            c.set_block((1, 1, 0), BlockType::Hi);
            c.set_block((1, 1, 1), BlockType::Hi);
            c.set_block((4, 2, 2), BlockType::Hi);

            c
        };

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
