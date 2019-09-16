use crate::block::Block;
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
            for i in 0u32..chunk::CHUNK_SIZE {
                c.set_block(0, i, i as i32, Block::Dirt);
            }
            c
        };

        Self {
            chunks: vec![chunk],
        }
    }
}

impl World {
    pub fn visible_chunks(&self) -> impl Iterator<Item=&Chunk> {
        // TODO filter visible
        self.chunks.iter()
    }
}
