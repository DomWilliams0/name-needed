use common::*;
use unit::dim::CHUNK_SIZE;
use unit::world::{
    BlockPosition, ChunkPosition, GlobalSliceIndex, SliceBlock, SliceIndex, WorldPoint,
    WorldPosition,
};

use crate::navigation::{
    AreaGraph, AreaNavEdge, AreaPath, BlockPath, NavigationError, WorldArea, WorldPath,
    WorldPathNode,
};

use crate::chunk::{BaseTerrain, Chunk};
use crate::loader::ChunkUpdate;
use crate::SliceRange;
#[cfg(any(test, feature = "benchmarking"))]
use crate::{chunk::ChunkDescriptor, loader::MemoryTerrainSource, WorldRef};

#[cfg(test)]
use crate::block::Block;
use std::ops::DerefMut;

#[cfg_attr(test, derive(Clone))]
pub struct World {
    chunks: Vec<Chunk>,
    area_graph: AreaGraph,
}

impl Default for World {
    fn default() -> Self {
        Self::empty()
    }
}

impl World {
    pub fn empty() -> Self {
        Self {
            chunks: Vec::new(),
            area_graph: AreaGraph::default(),
        }
    }

    pub fn all_chunks(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks.iter()
    }

    pub fn slice_bounds(&self) -> Option<SliceRange> {
        let min = self
            .chunks
            .iter()
            .map(|c| c.slice_bounds_as_slabs().bottom())
            .min();
        let max = self
            .chunks
            .iter()
            .map(|c| c.slice_bounds_as_slabs().top())
            .max();

        match (min, max) {
            (Some(min), Some(max)) => Some(SliceRange::from_bounds_unchecked(min, max)),
            _ => None,
        }
    }

    fn find_chunk_index(&self, chunk_pos: ChunkPosition) -> Option<usize> {
        self.chunks
            .binary_search_by_key(&chunk_pos, |c| c.pos())
            .ok()
    }

    pub(crate) fn find_chunk_with_pos(&self, chunk_pos: ChunkPosition) -> Option<&Chunk> {
        self.find_chunk_index(chunk_pos)
            .map(|idx| &self.chunks[idx])
    }

    pub fn find_chunk_with_pos_mut(&mut self, chunk_pos: ChunkPosition) -> Option<&mut Chunk> {
        self.find_chunk_index(chunk_pos)
            .map(move |idx| &mut self.chunks[idx])
    }

    pub(crate) fn find_area_path<F: Into<WorldPosition>, T: Into<WorldPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Result<AreaPath, NavigationError> {
        // resolve areas
        let resolve_area = |pos: WorldPosition| {
            let chunk_pos: ChunkPosition = pos.into();
            self.find_chunk_with_pos(chunk_pos)
                .and_then(|c| c.area_for_block(pos))
        };

        let from = from.into();
        let to = to.into();

        let from_area =
            resolve_area(from).ok_or_else(|| NavigationError::SourceNotWalkable(from))?;

        let to_area = resolve_area(to).ok_or_else(|| NavigationError::TargetNotWalkable(to))?;

        Ok(self.area_graph.find_area_path(from_area, to_area)?)
    }

    fn find_block_path<F: Into<BlockPosition>, T: Into<BlockPosition>>(
        &self,
        area: WorldArea,
        from: F,
        to: T,
    ) -> Result<BlockPath, NavigationError> {
        let block_graph = self
            .find_chunk_with_pos(area.chunk)
            .and_then(|c| c.block_graph_for_area(area))
            .ok_or_else(|| NavigationError::NoSuchArea(area))?;

        block_graph
            .find_block_path(from, to)
            .map_err(|e| NavigationError::BlockError(area, e))
    }

    /// Finds a path between 2 arbitrary positions in the world
    pub fn find_path<F: Into<WorldPosition>, T: Into<WorldPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Result<WorldPath, NavigationError> {
        let from_pos = from.into();
        let to_pos = to.into();

        let from = self
            .find_accessible_block_in_column_with_range(from_pos, None)
            .ok_or_else(|| NavigationError::SourceNotWalkable(from_pos))?;
        let to = self
            .find_accessible_block_in_column_with_range(to_pos, None)
            .ok_or_else(|| NavigationError::TargetNotWalkable(to_pos))?;

        // same blocks
        if from == to {
            return Err(NavigationError::ZeroLengthPath);
        }

        // find area path
        let area_path = self.find_area_path(from, to)?;

        // TODO optimize path with raytracing (#50)
        // TODO only calculate path for each area as needed (#51)

        // stupidly expand to block level path right now
        let mut full_path = Vec::with_capacity(CHUNK_SIZE.as_usize() / 2 * area_path.0.len()); // random estimate
        let mut start = BlockPosition::from(from);

        let convert_block_path = |area: WorldArea, path: BlockPath| {
            path.0.into_iter().map(move |n| WorldPathNode {
                block: n.block.to_world_position(area.chunk),
                exit_cost: n.exit_cost,
            })
        };

        for (a, b) in area_path.0.iter().tuple_windows() {
            // unwrap ok because all except the first are Some
            let b_entry = b.entry.unwrap();
            let exit = b_entry.exit_middle();

            // block path from last point to exiting this area
            let block_path = self.find_block_path(a.area, start, exit)?;
            full_path.extend(convert_block_path(a.area, block_path));

            // add transition edge from exit of this area to entering the next
            full_path.push(WorldPathNode {
                block: exit.to_world_position(a.area.chunk),
                exit_cost: b_entry.cost,
            });

            // continue from the entry point in the next chunk
            start = {
                let mut extended = b_entry.direction.extend_across_boundary(exit);

                extended.2 += b_entry.cost.z_offset();

                extended
            };
        }

        // final block path from entry of final area to goal
        let final_area = area_path.0.last().unwrap();
        let block_path = self.find_block_path(final_area.area, start, to)?;
        full_path.extend(convert_block_path(final_area.area, block_path));

        Ok(WorldPath::new(full_path, to))
    }

    /// Cheap check if an path exists between the 2 areas
    pub fn area_path_exists(&self, from: WorldArea, to: WorldArea) -> bool {
        self.area_graph.path_exists(from, to)
    }

    pub fn find_accessible_block_in_column(&self, x: i32, y: i32) -> Option<WorldPosition> {
        self.find_accessible_block_in_column_with_range(
            WorldPosition(x, y, SliceIndex::top()),
            None,
        )
    }

    pub fn find_accessible_block_in_column_with_range(
        &self,
        pos: WorldPosition,
        z_min: Option<GlobalSliceIndex>,
    ) -> Option<WorldPosition> {
        let chunk_pos = ChunkPosition::from(pos);
        let slice_block = SliceBlock::from(BlockPosition::from(pos));
        self.find_chunk_with_pos(chunk_pos)
            .and_then(|c| c.find_accessible_block(slice_block, Some(pos.2), z_min))
            .map(|pos| pos.to_world_position(chunk_pos))
    }

    pub(crate) fn add_loaded_chunk(
        &mut self,
        chunk: Chunk,
        area_nav: &[(WorldArea, WorldArea, AreaNavEdge)],
    ) {
        for area in chunk.areas() {
            self.area_graph.add_node(*area);
        }

        self.chunks.push(chunk);

        // maintain sorted chunk order
        self.chunks.sort_unstable_by_key(|c| c.pos());

        for &(src, dst, edge) in area_nav {
            self.area_graph.add_edge(src, dst, edge);
        }
    }

    pub fn apply_update(&mut self, update: ChunkUpdate) {
        let (chunk_pos, updates) = update;
        if let Some(chunk) = self.find_chunk_with_pos_mut(chunk_pos) {
            for (block_pos, opacities) in &updates {
                chunk
                    .raw_terrain_mut()
                    .with_block_mut_unchecked(*block_pos, |b| {
                        b.occlusion_mut()
                            .update_from_neighbour_opacities(*opacities);
                    })
            }

            // TODO only invalidate lighting
            chunk.invalidate();
        }
    }

    #[cfg(test)]
    pub(crate) fn block<P: Into<WorldPosition>>(&self, pos: P) -> Option<Block> {
        let pos = pos.into();
        self.find_chunk_with_pos(ChunkPosition::from(pos))
            .and_then(|chunk| chunk.get_block(pos))
    }
    #[cfg(test)]
    pub(crate) fn area_graph(&self) -> &AreaGraph {
        &self.area_graph
    }

    pub fn choose_random_walkable_block(&self, max_attempts: usize) -> Option<WorldPosition> {
        let mut rand = random::get();
        for _ in 0..max_attempts {
            let chunk = self.all_chunks().choose(rand.deref_mut()).unwrap(); // chunks wont be empty

            let x = rand.gen_range(0, CHUNK_SIZE.as_block_coord());
            let y = rand.gen_range(0, CHUNK_SIZE.as_block_coord());
            if let Some(block_pos) = chunk.find_accessible_block(SliceBlock(x, y), None, None) {
                return Some(block_pos.to_world_position(chunk.pos()));
            }
        }
        None
    }

    pub fn area<P: Into<WorldPosition>>(&self, pos: P) -> Option<WorldArea> {
        let block_pos = pos.into();
        let chunk_pos = ChunkPosition::from(block_pos);
        self.find_chunk_with_pos(chunk_pos)
            .and_then(|chunk| chunk.get_block(block_pos))
            .and_then(|block| block.chunk_area(block_pos.slice()))
            .map(|chunk_area| chunk_area.into_world_area(chunk_pos))
    }

    /// Returns first area that point.floor then ceil returns
    pub fn area_for_point(&self, point: WorldPoint) -> Option<(WorldPosition, WorldArea)> {
        point
            .floor_then_ceil()
            .find_map(|pos| self.area(pos).map(|area| (pos, area)))
    }
}

#[cfg(any(test, feature = "benchmarking"))]
pub fn world_from_chunks(chunks: Vec<ChunkDescriptor>) -> WorldRef {
    let source = MemoryTerrainSource::from_chunks(chunks.into_iter()).expect("bad chunks");
    world_from_preset(source)
}

#[cfg(any(test, feature = "benchmarking"))]
pub fn world_from_preset(mut source: MemoryTerrainSource) -> WorldRef {
    use crate::loader::{BlockingWorkerPool, TerrainSource, WorldLoader};
    use std::time::Duration;

    let chunks_pos = source.all_chunks();

    // TODO build area graph in loader
    // let area_graph = AreaGraph::from_chunks(&[]);

    let mut loader = WorldLoader::new(source, BlockingWorkerPool::default());
    for pos in chunks_pos {
        loader.request_chunk(pos);
        let _ = loader.block_on_next_finalization(Duration::from_secs(20));
    }

    // apply all chunk updates
    let updates = loader.chunk_updates_rx().unwrap();
    while let Ok(update) = updates.try_recv() {
        loader.world().borrow_mut().apply_update(update);
    }

    loader.world()
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use unit::world::{BlockPosition, ChunkPosition, GlobalSliceIndex};

    use crate::block::BlockType;
    use crate::chunk::ChunkBuilder;
    use crate::navigation::EdgeCost;
    use crate::presets;
    use crate::world::world_from_chunks;
    use common::Itertools;

    #[test]
    fn world_path_single_block_in_y_direction() {
        let w = world_from_chunks(vec![ChunkBuilder::new()
            .fill_slice(1, BlockType::Grass)
            .build((0, 0))])
        .into_inner();

        let path = w
            .find_path((2, 2, 2), (3, 3, 2))
            .expect("path should succeed");

        assert_eq!(path.path().len(), 2);
    }

    #[test]
    fn accessible_block_in_column() {
        let w = world_from_chunks(vec![ChunkBuilder::new()
            .fill_slice(6, BlockType::Grass) // lower slice
            .fill_slice(9, BlockType::Grass) // higher slice blocks it...
            .set_block((10, 10, 9), BlockType::Air) // ...with a hole here
            .build((0, 0))])
        .into_inner();

        // finds higher slice
        assert_eq!(
            w.find_accessible_block_in_column(4, 4),
            Some((4, 4, 10).into())
        );

        // ...but not when we start searching from a lower point
        assert_eq!(
            w.find_accessible_block_in_column_with_range((4, 4, 8).into(), None),
            Some((4, 4, 7).into())
        );

        // ...or when the lower bound is set to exactly that
        assert_eq!(
            w.find_accessible_block_in_column_with_range(
                (4, 4, 8).into(),
                Some(GlobalSliceIndex::new(7))
            ),
            Some((4, 4, 7).into())
        );

        // even when starting from the slice itself
        assert_eq!(
            w.find_accessible_block_in_column_with_range((4, 4, 7).into(), None),
            Some((4, 4, 7).into())
        );

        // finds lower slice through hole
        assert_eq!(
            w.find_accessible_block_in_column(10, 10),
            Some((10, 10, 7).into())
        );

        // ...but not when the lower bound prevents it
        assert_eq!(
            w.find_accessible_block_in_column_with_range(
                (10, 10, 20).into(),
                Some(GlobalSliceIndex::new(8))
            ),
            None
        );

        // non existent
        assert!(w.find_accessible_block_in_column(-5, 30).is_none());
    }

    #[test]
    fn world_path_within_area() {
        let world = world_from_chunks(vec![ChunkBuilder::new()
            .fill_slice(2, BlockType::Stone)
            .set_block((0, 0, 3), BlockType::Grass)
            .set_block((8, 8, 3), BlockType::Grass)
            .build((0, 0))])
        .into_inner();

        let path = world
            .find_path((0, 0, 4), (8, 8, 4))
            .expect("path should succeed");

        assert_eq!(path.path().first().unwrap().exit_cost, EdgeCost::JumpDown);
        assert_eq!(path.path().last().unwrap().exit_cost, EdgeCost::JumpUp);
    }

    #[test]
    fn world_path_cross_areas() {
        use common::LevelFilter;
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        // cross chunks
        let world = world_from_chunks(vec![
            ChunkBuilder::new()
                .fill_slice(4, BlockType::Grass)
                .build((3, 5)),
            ChunkBuilder::new()
                .fill_slice(4, BlockType::Grass)
                .build((4, 5)),
            ChunkBuilder::new()
                .fill_slice(5, BlockType::Grass)
                .build((5, 5)),
            ChunkBuilder::new()
                .fill_slice(6, BlockType::Grass)
                .build((6, 5)),
            ChunkBuilder::new()
                .fill_slice(6, BlockType::Grass)
                .build((6, 4)),
            ChunkBuilder::new()
                .fill_slice(6, BlockType::Grass)
                .build((6, 3)),
            ChunkBuilder::new().build((0, 0)),
        ])
        .into_inner();

        let from = BlockPosition::from((0, 2, 5)).to_world_position((3, 5));
        let to = BlockPosition::from((5, 8, 7)).to_world_position((6, 3));

        let path = world.find_path(from, to).expect("path should succeed");
        assert_eq!(path.target(), to);

        // all should be adjacent
        for (a, b) in path.path().iter().tuple_windows() {
            eprintln!("{:?} {:?}", a, b);
            let dx = b.block.0 - a.block.0;
            let dy = b.block.1 - a.block.1;

            assert_eq!((dx + dy).abs(), 1);
        }

        // expect 2 jumps
        assert_eq!(
            path.path()
                .iter()
                .filter(|b| b.exit_cost == EdgeCost::JumpUp)
                .count(),
            2
        );
    }

    #[test]
    fn ring_path() {
        use common::LevelFilter;
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();
        let world = world_from_chunks(presets::ring()).into_inner();

        let src = BlockPosition(5, 5, GlobalSliceIndex::top()).to_world_position((0, 1));
        let dst = BlockPosition(5, 5, GlobalSliceIndex::top()).to_world_position((-1, 1));

        let _ = world.find_path(src, dst).expect("path should succeed");
    }

    #[test]
    fn find_chunk() {
        let world = world_from_chunks(vec![
            ChunkBuilder::new().build((0, 0)),
            ChunkBuilder::new().build((1, 0)),
            ChunkBuilder::new().build((2, 0)),
            ChunkBuilder::new().build((3, 0)),
            ChunkBuilder::new().build((0, 1)),
            ChunkBuilder::new().build((0, -3)),
            ChunkBuilder::new().build((2, 5)),
        ])
        .into_inner();

        for chunk in world.all_chunks() {
            assert_eq!(
                world.find_chunk_with_pos(chunk.pos()).unwrap().pos(),
                chunk.pos()
            );
        }

        assert!(world.find_chunk_with_pos(ChunkPosition(10, 10)).is_none());
    }
}
