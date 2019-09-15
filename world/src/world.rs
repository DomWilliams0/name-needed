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
            let mut c = Chunk::empty((0,0));

            // ground except for corner
            for b in c.slice_mut(0).iter_mut().skip(1) {
                *b = Block::Dirt;
            }

            // interesting terrain
            let mut one_up = c.slice_mut(1);
            for x in 1..chunk::CHUNK_SIZE - 1 {
                for y in 1..chunk::CHUNK_SIZE - 1 {
                    one_up.set_block(x, y, Block::Dirt);
                }
            }
            c.set_block(1, 2, 2, Block::Dirt);

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
