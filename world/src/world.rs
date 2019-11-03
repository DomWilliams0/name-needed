use std::collections::HashSet;

use itertools::Itertools;
use log::{debug, warn};

use crate::area::{AreaGraph, AreaPath, WorldArea, WorldPath};
use crate::chunk::Chunk;
use crate::coordinate::world::{SliceBlock, WorldPosition};
use crate::{presets, BlockPosition, ChunkPosition, SliceRange};

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

    pub fn all_chunks(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks.iter()
    }

    pub fn visible_chunks(&self) -> impl Iterator<Item = &Chunk> {
        // TODO filter visible
        self.all_chunks()
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

    fn find_chunk<P: Fn(&Chunk) -> bool>(&self, predicate: P) -> Option<&Chunk> {
        // TODO spatial
        self.chunks.iter().find(|c| predicate(c))
    }

    fn find_chunk_with_pos(&self, chunk_pos: ChunkPosition) -> Option<&Chunk> {
        // TODO spatial
        self.find_chunk(|c| c.pos() == chunk_pos)
    }

    fn chunk_for_area(&self, area: WorldArea) -> Option<&Chunk> {
        self.find_chunk(|c| c.pos() == area.chunk)
    }

    pub(crate) fn find_area_path<F: Into<WorldPosition>, T: Into<WorldPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Option<AreaPath> {
        // resolve areas
        let resolve_area = |pos: WorldPosition| {
            let chunk_pos: ChunkPosition = pos.into();
            self.find_chunk_with_pos(chunk_pos)
                .and_then(|c| c.area_for_block(pos))
        };

        let (from_area, to_area) = match (resolve_area(from.into()), resolve_area(to.into())) {
            (Some(a), Some(b)) => (a, b),
            _ => return None,
        };

        self.area_graph.find_area_path(from_area, to_area)
    }
    /// Finds a path between 2 arbitrary positions in the world
    pub fn find_path<F: Into<WorldPosition>, T: Into<WorldPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Option<WorldPath> {
        let from: WorldPosition = from.into();
        let to: WorldPosition = to.into();

        // find area path
        let area_path = match self.find_area_path(from, to) {
            Some(path) => path,
            None => return None,
        };

        // TODO optimize path with raytracing (#50)
        // TODO only calculate path for each area as needed (#51)

        // stupidly expand to block level path right now
        let block_path = area_path
            .into_iter()
            .flat_map(|node| {
                let chunk = self.chunk_for_area(node.area)
                    .expect("area should be valid");
                let block_graph = chunk.block_graph_for_area(node.area).unwrap();

                let start = node.entry.map(|(pos, _cost)| pos).unwrap_or(from);
                let end = node.exit.map(|(pos, _cost)| pos).unwrap_or(to);

                let path = block_graph
                    .find_path(start, end)
                    .expect("block path should exist");

                // convert to world pos
                path.into_iter()
                    .map(move |(pos, cost)| (pos.to_world_pos(chunk.pos()), cost))
            })
            .collect_vec();

        Some(WorldPath(block_path))
    }

    pub fn find_accessible_block_in_column(&self, x: i32, y: i32) -> Option<WorldPosition> {
        let world_pos = WorldPosition(x, y, 0);
        let chunk_pos = ChunkPosition::from(world_pos);
        let slice_block = SliceBlock::from(BlockPosition::from(world_pos));
        self.find_chunk_with_pos(chunk_pos).and_then(|c| {
            c.slices_from_top()
                .find(|(_, slice)| slice[slice_block].walkable())
                .map(|(z, _)| slice_block.to_block_position(z).to_world_pos(chunk_pos))
        })
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use matches::assert_matches;

    use crate::area::{AreaPathNode, EdgeCost, WorldArea};
    use crate::block::{BlockHeight, BlockType};
    use crate::coordinate::world::WorldPosition;
    use crate::{ChunkBuilder, ChunkPosition, World};

    #[test]
    fn area_path_cross_three_chunks() {
        let w = World::from_chunks(vec![
            ChunkBuilder::new()
                .set_block((14, 2, 1), BlockType::Stone)
                .set_block((15, 2, 1), BlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(1, BlockType::Grass)
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 5, 2), BlockType::Stone)
                .set_block((1, 5, 3), BlockType::Stone)
                .build((1, 0)),
        ]);
        let src = (-2, 2, 2);
        let dst = (17, 5, 4);

        let path = w.find_area_path(
            src, // chunk -1, 0
            dst, // chunk 1, 0
        ).expect("path should succeed");

        assert_eq!(path.0.len(), 3);

        let mut p = path.0.iter();
        assert_matches!(
            p.next(),
            Some(AreaPathNode {
                area:
                    WorldArea {
                        chunk: ChunkPosition(-1, 0),
                        ..
                    },
                entry: None,
                exit: Some((WorldPosition(-1, 2, 2), EdgeCost::Walk)),
            })
        );

        assert_matches!(
            p.next(),
            Some(AreaPathNode {
                area:
                    WorldArea {
                        chunk: ChunkPosition(0, 0),
                        ..
                    },
                entry: Some((WorldPosition(0, 2, 2), EdgeCost::Walk)),
                exit: Some((WorldPosition(15, _, 2), EdgeCost::JumpUp)),
            })
        );

        assert_matches!(
            p.next(),
            Some(AreaPathNode {
                area:
                    WorldArea {
                        chunk: ChunkPosition(1, 0),
                        ..
                    },
                entry: Some((WorldPosition(16, _, 3), EdgeCost::JumpUp)),
                exit: None,
            })
        );

        // now find block path
        w.find_path(src, dst).expect("block path should succeed");
    }

    #[test]
    fn area_path_cross_two_chunks() {
        let w = World::from_chunks(vec![
            ChunkBuilder::new()
                .set_block((14, 2, 1), BlockType::Stone)
                .set_block((15, 2, 1), BlockType::Stone)
                .build((-1, 0)),
            ChunkBuilder::new()
                .fill_slice(1, BlockType::Grass)
                .build((0, 0)),
        ]);

        let path = w.find_area_path(
            (-2, 2, 2),  // chunk -1, 0
            (10, 10, 2), // chunk 0, 0
        ).expect("path should succeed");

        assert_eq!(path.0.len(), 2);

        let mut p = path.into_iter();
        assert_matches!(
            p.next(),
            Some(AreaPathNode {
                area:
                    WorldArea {
                        chunk: ChunkPosition(-1, 0),
                        ..
                    },
                entry: None,
                exit: Some((WorldPosition(-1, 2, 2), EdgeCost::Walk)),
            })
        );

        assert_matches!(
            p.next(),
            Some(AreaPathNode {
                area:
                    WorldArea {
                        chunk: ChunkPosition(0, 0),
                        ..
                    },
                entry: Some((WorldPosition(0, _, 2), EdgeCost::Walk)),
                exit: None,
            })
        );
    }

    #[test]
    fn area_path_within_single_chunk() {
        let w = World::from_chunks(vec![
            ChunkBuilder::new()
                .fill_slice(1, BlockType::Grass)
                .build((0, 0)),
        ]);

        let path = w.find_area_path(
            (2, 2, 2), // chunk 0, 0
            (8, 3, 2), // also chunk 0, 0
        ).expect("path should succeed");

        assert_eq!(path.0.len(), 1);

        assert_matches!(
            path.0.first(),
            Some(AreaPathNode {
                area:
                    WorldArea {
                        chunk: ChunkPosition(0, 0),
                        ..
                    },
                entry: None,
                exit: None,
            })
        );
    }

    #[test]
    fn world_path_single_block_in_y_direction() {
        let w = World::from_chunks(vec![
            ChunkBuilder::new()
                .fill_slice(1, BlockType::Grass)
                .build((0, 0)),
        ]);

        let path = w.find_path((2, 2, 2), (3, 3, 2))
            .expect("path should succeed");

        assert_matches!(path.0.len(), 2);
    }

    #[test]
    fn world_path_hippity_hoppity() {
        let w = World::from_chunks(vec![
            ChunkBuilder::new()
                .set_block((0, 0, 0), (BlockType::Dirt, BlockHeight::Full))
                .set_block((0, 1, 1), (BlockType::Dirt, BlockHeight::Half)) // half step up
                .set_block((0, 2, 1), (BlockType::Dirt, BlockHeight::Full)) // half step up
                .set_block((0, 3, 2), (BlockType::Dirt, BlockHeight::Half)) // half step up
                .set_block((0, 4, 2), (BlockType::Dirt, BlockHeight::Full)) // half step up
                .set_block((0, 5, 3), (BlockType::Dirt, BlockHeight::Full)) // jump up
                .build((0, 0)),
        ]);

        let path = w.find_path((0, 0, 1), (0, 5, 4))
            .expect("path should succeed");

        println!("{:#?}", path);

        let mut p = path.into_iter();

        // half step from 0,0 to 0,1
        assert_matches!(p.next(), Some((WorldPosition(0, 1, 1), EdgeCost::Step(_))));

        // half step again from 0,1 (half) to 0,2 (full)
        assert_matches!(p.next(), Some((WorldPosition(0, 2, 2), EdgeCost::Step(_))));

        // half step again to 0,3
        assert_matches!(p.next(), Some((WorldPosition(0, 3, 2), EdgeCost::Step(_))));

        // half step again to 0,4
        assert_matches!(p.next(), Some((WorldPosition(0, 4, 3), EdgeCost::Step(_))));

        // jump to 0,5
        assert_matches!(p.next(), Some((WorldPosition(0, 5, 4), EdgeCost::JumpUp)));

        // done
        assert_matches!(p.next(), None);
    }

    #[test]
    fn accessible_block_in_column() {
        let w = World::from_chunks(vec![
            ChunkBuilder::new()
                .fill_slice(6, BlockType::Grass) // lower slice
                .fill_slice(9, BlockType::Grass) // higher slice blocks it...
                .set_block((10, 10, 9), BlockType::Air) // ...with a hole here
                .build((0, 0)),
        ]);

        // finds higher slice
        assert_eq!(
            w.find_accessible_block_in_column(4, 4),
            Some(WorldPosition(4, 4, 10))
        );

        // finds lower slice through hole
        assert_eq!(
            w.find_accessible_block_in_column(10, 10),
            Some(WorldPosition(10, 10, 7))
        );

        // non existent
        assert_matches!(w.find_accessible_block_in_column(-5, 30), None);
    }
}
