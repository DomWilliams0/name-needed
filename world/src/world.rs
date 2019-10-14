use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use log::{debug, warn};

use crate::area::AreaGraph;
use crate::chunk::Chunk;

use crate::{presets, SliceRange};

/// Reference to the world
pub type WorldRef = Rc<RefCell<World>>;

pub fn world_ref(w: World) -> WorldRef {
    Rc::new(RefCell::new(w))
}

pub struct World {
    chunks: Vec<Chunk>,
    area_graph: AreaGraph,
}

impl Default for World {
    fn default() -> Self {
        presets::multi_chunk_wonder()
    }
}

impl World {
    pub(crate) fn from_chunks(chunks: Vec<Chunk>) -> Self {
        // ensure all are unique
        {
            let mut seen = HashSet::new();
            let mut bad = Vec::with_capacity(chunks.len());
            for c in &chunks {
                if !seen.insert(c.pos()) {
                    bad.push(c.pos())
                }
            }

            if !bad.is_empty() {
                for bad_pos in &bad {
                    warn!("duplicate chunk {:?} in world is not allowed", bad_pos);
                }

                panic!("[world] {} duplicate chunks!!1!", bad.len()); // TODO return a result instead
            }
        }

        debug!("world has {} chunks", chunks.len());

        // build area graph
        let area_graph = AreaGraph::from_chunks(&chunks);

        Self { chunks, area_graph }
    }

    pub fn visible_chunks(&self) -> impl Iterator<Item = &Chunk> {
        // TODO filter visible
        self.chunks.iter()
    }

    pub fn slice_bounds(&self) -> SliceRange {
        let min = self.chunks
            .iter()
            .map(|c| c.slice_bounds_as_slabs().bottom())
            .min();
        let max = self.chunks
            .iter()
            .map(|c| c.slice_bounds_as_slabs().top())
            .max();

        match (min, max) {
            (Some(min), Some(max)) => SliceRange::from_bounds(min, max),
            _ => SliceRange::null(),
        }
    }

    /*
        /// Finds a path between 2 arbitrary positions in the world
        pub fn find_path<F: Into<WorldPosition>, T: Into<WorldPosition>>(
            &self,
            from: F,
            to: T,
        ) -> Option<Path> {
            None
        }
    */
}

#[cfg(test)]
mod tests {
    /*
            #[test]
            fn find_path_cross_chunk_simple() {
                // 2 chunks with a line across x at y=0
                let w = World::from_chunks(vec![
                    ChunkBuilder::new()
                        .fill_range((0, 0, 0), (CHUNK_SIZE as u16 - 1, 1, 1), |_| {
                            Some(BlockType::Stone)
                        })
                        .build((-1, 0)),
                    ChunkBuilder::new()
                        .fill_range((0, 0, 0), (CHUNK_SIZE as u16 - 1, 1, 1), |_| {
                            Some(BlockType::Stone)
                        })
                        .build((0, 0)),
                ]);

                let path = w.find_path(
                    (-4, 0, 1), // chunk (-1, 0)
                    (4, 0, 1),  // chunk (0, 0)
                );

                assert!(path.is_some());
            }
        */
}
