use std::collections::HashSet;
use std::iter::once;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::broadcast;

use misc::derive_more::Constructor;
use misc::*;
use unit::world::{
    BlockCoord, BlockPosition, ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex,
    SlabLocation, SliceBlock, SliceIndex, WorldPosition, WorldPositionRange,
};
use unit::world::{WorldPoint, CHUNK_SIZE};

use crate::block::{Block, BlockDurability};
use crate::chunk::affected_neighbours::OcclusionAffectedNeighbourSlabs;
use crate::chunk::slab::SliceNavArea;
use crate::chunk::slice_navmesh::SliceAreaIndex;
use crate::chunk::{AreaInfo, BlockDamageResult, Chunk, SlabData, SlabThingOrWait, SparseGrid};
use crate::context::WorldContext;
use crate::loader::{SlabTerrainUpdate, SlabVerticalSpace};
use crate::navigation::{
    AreaGraph, AreaGraphSearchContext, AreaNavEdge, AreaPath, BlockGraph, BlockGraphSearchContext,
    BlockPath, ExploreResult, NavigationError, SearchGoal, WorldArea, WorldPath, WorldPathNode,
};
use crate::navigationv2::world_graph::WorldGraph;
use crate::navigationv2::{as_border_area, ChunkArea, NavRequirement, SlabArea};
use crate::neighbour::{NeighbourOffset, WorldNeighbours};
use crate::occlusion::NeighbourOpacity;
use crate::{BlockOcclusion, BlockType, OcclusionFace, Slab, SliceRange, WorldAreaV2, WorldRef};

/// All mutable world changes must go through `loader.apply_terrain_updates`
pub struct World<C: WorldContext> {
    chunks: Vec<Chunk<C>>,
    #[deprecated]
    area_graph: AreaGraph,
    nav_graph: WorldGraph,
    load_notifier: LoadNotifier,
}

#[derive(Clone)]
pub struct LoadNotifier {
    send: broadcast::Sender<SlabLocation>,
    /// Dont bother sending anything if there are no receivers
    waiters: Arc<AtomicUsize>,
}

/// Will receive all broadcasted events while this is alive
pub struct ListeningLoadNotifier {
    /// Decrement on drop
    waiters: Arc<AtomicUsize>,
    recv: broadcast::Receiver<SlabLocation>,
}

pub enum WaitResult {
    Success,
    /// Channel is disconnected
    Disconnected,
    /// Channel is lagging, check slab state again and wait again if necessary
    Retry,
}

#[derive(Constructor)]
pub struct WorldChangeEvent {
    pub slab: SlabLocation,
}

impl<C: WorldContext> Default for World<C> {
    fn default() -> Self {
        Self::empty()
    }
}

pub enum AreaLookup {
    /// Block doesn't exist
    BadPosition,

    /// Block exists but has no area
    NoArea,

    /// Block has area
    Area(WorldArea),
}

struct ContiguousChunkIteratorMut<'a, C: WorldContext> {
    world: &'a mut World<C>,
    last_chunk: Option<(ChunkLocation, usize)>,
}

pub(crate) struct ContiguousChunkIterator<'a, C: WorldContext> {
    world: &'a World<C>,
    last_chunk: Option<(ChunkLocation, Option<usize>)>,
    #[cfg(test)]
    matched_last: bool,
}

/// Abort an exploration path-find early
pub struct ExplorationFilter(pub Box<dyn (Fn(WorldPosition) -> ExplorationResult) + Send + Sync>);

// only used on main thread by synchronous systems
unsafe impl Send for ExplorationFilter {}
unsafe impl Sync for ExplorationFilter {}

pub enum ExplorationResult {
    Continue,
    Abort,
}

impl<C: WorldContext> World<C> {
    pub fn empty() -> Self {
        Self {
            chunks: Vec::new(),
            area_graph: AreaGraph::default(),
            nav_graph: WorldGraph::default(),
            load_notifier: LoadNotifier::default(),
        }
    }

    pub fn all_chunks(&self) -> impl Iterator<Item = &Chunk<C>> {
        self.chunks.iter()
    }

    pub fn slice_bounds(&self) -> Option<SliceRange> {
        let slab_ranges = self.chunks.iter().map(|c| c.terrain().slab_range());

        let min = slab_ranges.clone().map(|(bottom, _)| bottom).min();
        let max = slab_ranges.map(|(_, top)| top).max();

        min.zip(max).map(|(min_slab, max_slab)| {
            let min_slice = LocalSliceIndex::bottom().to_global(min_slab);
            let max_slice = LocalSliceIndex::top().to_global(max_slab);
            SliceRange::from_bounds_unchecked(min_slice, max_slice)
        })
    }

    fn find_chunk_index(&self, chunk_pos: ChunkLocation) -> Result<usize, usize> {
        self.chunks.binary_search_by_key(&chunk_pos, |c| c.pos())
    }

    pub fn has_slab(&self, slab_pos: SlabLocation) -> bool {
        self.find_chunk_with_pos(slab_pos.chunk)
            .map(|chunk| chunk.terrain().slab(slab_pos.slab).is_some())
            .unwrap_or_default()
    }

    pub fn get_slab_mut(&mut self, slab_pos: SlabLocation) -> Option<&mut Slab<C>> {
        self.find_chunk_with_pos_mut(slab_pos.chunk)
            .and_then(|chunk| chunk.terrain_mut().slab_mut(slab_pos.slab))
    }

    // TODO lru cache for chunk lookups
    pub fn find_chunk_with_pos(&self, chunk_pos: ChunkLocation) -> Option<&Chunk<C>> {
        self.find_chunk_index(chunk_pos)
            .ok()
            .map(|idx| &self.chunks[idx])
    }

    pub fn find_chunk_with_pos_mut(&mut self, chunk_pos: ChunkLocation) -> Option<&mut Chunk<C>> {
        self.find_chunk_index(chunk_pos)
            .ok()
            .map(move |idx| &mut self.chunks[idx])
    }

    pub(crate) fn find_area_path<F: Into<WorldPosition>, T: Into<WorldPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Result<AreaPath, NavigationError> {
        // resolve areas
        let resolve_area = |pos: WorldPosition| {
            let chunk_pos: ChunkLocation = pos.into();
            self.find_chunk_with_pos(chunk_pos)
                .and_then(|c| c.area_for_block(pos.into()))
        };

        let from = from.into();
        let to = to.into();

        let from_area = resolve_area(from).ok_or(NavigationError::SourceNotWalkable(from))?;

        let to_area = resolve_area(to).ok_or(NavigationError::TargetNotWalkable(to))?;

        Ok(self
            .area_graph
            .find_area_path(from_area, to_area, unimplemented!())?)
    }

    fn find_block_path(
        &self,
        area: WorldArea,
        from: BlockPosition,
        to: BlockPosition,
        target: SearchGoal,
    ) -> Result<BlockPath, NavigationError> {
        let block_graph = self
            .find_chunk_with_pos(area.chunk)
            .and_then(|c| c.block_graph_for_area(area))
            .ok_or(NavigationError::NoSuchArea(area))?;

        block_graph
            .find_block_path(from, to, target, unreachable!())
            .map_err(|e| NavigationError::BlockError(area, e))
    }

    /// Finds a path between 2 arbitrary positions in the world
    pub fn find_path<F: Into<WorldPosition>, T: Into<WorldPosition>>(
        &self,
        from: F,
        to: T,
    ) -> Result<WorldPath, NavigationError> {
        self.find_path_with_goal(from.into(), to.into(), SearchGoal::Arrive)
    }

    pub fn find_path_with_goal(
        &self,
        from: WorldPosition,
        to: WorldPosition,
        goal: SearchGoal,
    ) -> Result<WorldPath, NavigationError> {
        let from = self
            .find_accessible_block_in_column_with_range(from, None)
            .ok_or(NavigationError::SourceNotWalkable(from))?;

        let to_accessible = self.find_accessible_block_in_column_with_range(to, None);
        let (to, goal) = match goal {
            SearchGoal::Arrive | SearchGoal::Nearby(_) => {
                // target block must be accessible
                to_accessible.map(|pos| (pos, goal))
            }
            SearchGoal::Adjacent => {
                // only need to be adjacent, try neighbours first
                let neighbours = WorldNeighbours::new(to)
                    .chain(WorldNeighbours::new(to.above()))
                    .chain(WorldNeighbours::new(to.below()));

                let accessible_neighbour = neighbours
                    .filter_map(|pos| {
                        self.find_accessible_block_in_column_with_range(pos, Some(pos.2 - 1))
                    })
                    .min_by_key(|pos| pos.distance2(from));

                if let Some(neighbour) = accessible_neighbour {
                    // arrive at closest neighbour
                    Some((neighbour, SearchGoal::Arrive))
                } else {
                    // arrive adjacent to target
                    to_accessible.map(|pos| (pos, SearchGoal::Arrive))
                }
            }
        }
        .ok_or(NavigationError::TargetNotWalkable(to))?;

        // same blocks
        if from == to {
            return Ok(WorldPath::new(Vec::new(), to));
        }

        // find area path
        let area_path = self.find_area_path(from, to)?;

        // TODO optimize path with raytracing (#50)
        // TODO only calculate path for each area as needed (#51)

        // stupidly expand to block level path right now
        let mut full_path = Vec::with_capacity(CHUNK_SIZE.as_usize() / 2 * area_path.0.len()); // random estimate
        let mut start = BlockPosition::from(from);

        for (a, b) in area_path.0.iter().tuple_windows() {
            // unwrap ok because all except the first are Some
            let b_entry: AreaNavEdge = b.entry.unwrap();
            let exit = b_entry.exit_closest(start);

            // block path from last point to exiting this area
            let block_path = self.find_block_path(a.area, start, exit, SearchGoal::Arrive)?;
            full_path.extend(Self::convert_block_path(a.area, block_path));

            // add transition edge from exit of this area to entering the next
            full_path.push(WorldPathNode {
                block: exit.to_world_position(a.area.chunk),
                exit_cost: b_entry.cost,
            });

            // continue from the entry point in the next chunk
            start = {
                let extended = b_entry.direction.extend_across_boundary_aligned(exit);
                extended.above_by(b_entry.cost.z_offset())
            };
        }

        // final block path from entry of final area to goal
        let final_area = area_path.0.last().unwrap();
        let block_path = self.find_block_path(final_area.area, start, to.into(), goal)?;
        let real_target = block_path.target.to_world_position(final_area.area.chunk);
        full_path.extend(Self::convert_block_path(final_area.area, block_path));

        Ok(WorldPath::new(full_path, real_target))
    }

    fn convert_block_path(area: WorldArea, path: BlockPath) -> impl Iterator<Item = WorldPathNode> {
        path.path.into_iter().map(move |n| WorldPathNode {
            block: n.block.to_world_position(area.chunk),
            exit_cost: n.exit_cost,
        })
    }

    /// Meanders randomly, using the given amount of fuel. Doesn't calculate a path
    pub fn find_exploratory_destination(
        &self,
        from: WorldPosition,
        mut fuel: u32,
        filter: Option<ExplorationFilter>,
    ) -> Result<WorldPosition, NavigationError> {
        let (from, from_area) = self
            .find_accessible_block_in_column_with_range(from, None)
            .and_then(|pos| self.area(from).ok().map(|area| (pos, area)))
            .ok_or(NavigationError::SourceNotWalkable(from))?;

        let mut random = thread_rng();

        // loop vars
        let mut current_pos = from;
        let mut current_area = from_area;
        let mut current_final_target = from;
        loop {
            let current_chunk = ChunkLocation::from(current_pos);
            let block_graph = self
                .find_chunk_with_pos(current_chunk)
                .and_then(|c| c.block_graph_for_area(current_area))
                .ok_or(NavigationError::NoSuchArea(current_area))?;

            // explore within current chunk, possibly ending early at a border
            let (explore_result, target_block) = block_graph.explore(
                current_pos.into(),
                &mut fuel,
                unreachable!(),
                &mut random,
                filter.as_ref().map(|func| (func, current_chunk)),
            );

            current_final_target = match target_block {
                Some(block) => block.to_world_position(current_pos),
                None => break,
            };

            // fuel is exhausted or search is otherwise ended
            if fuel == 0 || !matches!(explore_result, ExploreResult::AtBorder) {
                break;
            }

            // determine if we should continue into the next area
            let (next_block, next_area) = match self
                .find_candidate_chunks_to_explore(current_final_target)
                .collect::<ArrayVec<_, 3>>()
                .choose(&mut random)
            {
                Some(next) => *next,
                _ => break,
            };

            current_pos = next_block;
            current_area = next_area;
        }

        Ok(current_final_target)
    }

    fn find_candidate_chunks_to_explore(
        &self,
        exiting_block: WorldPosition,
    ) -> impl Iterator<Item = (WorldPosition, WorldArea)> + '_ {
        let exiting_block_pos = BlockPosition::from(exiting_block);
        let src_area = self.area(exiting_block).ok().expect("bad src"); // TODO

        NeighbourOffset::accessible_neighbours(exiting_block_pos).filter_map(move |offset| {
            let (tgt_block, tgt_chunk) =
                offset.extend_across_any_boundary(exiting_block_pos, exiting_block.into());
            let tgt_area = self
                .find_chunk_with_pos(tgt_chunk)
                .and_then(|c| c.area_for_block(tgt_block))?;

            let edge = self.area_graph.get_adjacent_area_edge(src_area, tgt_area)?;
            if edge.contains(exiting_block_pos) {
                let tgt_block = tgt_block.to_world_position(tgt_chunk);
                Some((tgt_block, tgt_area))
            } else {
                None
            }
        })
    }

    /// Cheap check if an area path exists between the areas of the 2 blocks
    pub fn path_exists(&self, from: WorldPosition, to: WorldPosition) -> bool {
        self.area(from)
            .ok()
            .and_then(|from| self.area(to).ok().map(|to| (from, to)))
            .map(|(from, to)| self.area_path_exists(from, to))
            .unwrap_or(false)
    }

    /// Cheap check if an path exists between the 2 areas
    pub fn area_path_exists(&self, from: WorldArea, to: WorldArea) -> bool {
        self.area_graph.path_exists(from, to, unreachable!())
    }

    /// Searches downward
    pub fn find_area_for_block(
        &self,
        block: WorldPosition,
        requirement: NavRequirement,
    ) -> Option<crate::navigationv2::world_graph::WorldArea> {
        let chunk_loc = ChunkLocation::from(block);
        let chunk = self.find_chunk_with_pos(chunk_loc)?;

        chunk
            .find_area_for_block_with_height(block.into(), requirement)
            .map(
                |(slab_area, _)| crate::navigationv2::world_graph::WorldArea {
                    chunk_idx: chunk_loc,
                    chunk_area: ChunkArea {
                        slab_idx: block.slice().into(),
                        slab_area,
                    },
                },
            )
    }

    pub fn lookup_area_info(
        &self,
        area: crate::navigationv2::world_graph::WorldArea,
    ) -> Option<AreaInfo> {
        self.find_chunk_with_pos(area.chunk_idx)
            .and_then(|c| c.area_info(area.chunk_area.slab_idx, area.chunk_area.slab_area))
    }

    #[deprecated]
    pub fn find_accessible_block_in_column(&self, x: i32, y: i32) -> Option<WorldPosition> {
        self.find_accessible_block_in_column_with_range(
            WorldPosition(x, y, SliceIndex::top()),
            None,
        )
    }

    #[deprecated]
    pub fn find_accessible_block_in_column_with_range(
        &self,
        pos: WorldPosition,
        z_min: Option<GlobalSliceIndex>,
    ) -> Option<WorldPosition> {
        unreachable!();
        let chunk_pos = ChunkLocation::from(pos);
        let slice_block = SliceBlock::from(BlockPosition::from(pos));
        self.find_chunk_with_pos(chunk_pos)
            .and_then(|c| {
                c.terrain()
                    .find_accessible_block(slice_block, Some(pos.2), z_min)
            })
            .map(|pos| pos.to_world_position(chunk_pos))
    }

    #[deprecated]
    pub fn find_accessible_block_in_range(
        &self,
        range: &WorldPositionRange,
    ) -> Option<WorldPosition> {
        let mut chunks = ContiguousChunkIterator::new(self);
        let (_, _, (min_z, max_z)) = range.ranges();

        for (x, y) in range.iter_columns() {
            let pos = WorldPosition::new(x, y, GlobalSliceIndex::new(max_z));
            let chunk_pos = ChunkLocation::from(pos);
            let slice_block = SliceBlock::from(BlockPosition::from(pos));
            let res = chunks
                .next(chunk_pos)
                .and_then(|c| {
                    c.terrain().find_accessible_block(
                        slice_block,
                        Some(pos.2),
                        Some(GlobalSliceIndex::new(min_z)),
                    )
                })
                .map(|pos| pos.to_world_position(chunk_pos));

            if res.is_some() {
                return res;
            }
        }

        None
    }

    pub(crate) fn ensure_chunk(&mut self, chunk: ChunkLocation) -> &mut Chunk<C> {
        let idx = match self.find_chunk_index(chunk) {
            Ok(idx) => idx,
            Err(idx) => {
                debug!("adding empty chunk"; chunk);
                self.chunks
                    .insert(idx, Chunk::empty_with_world(self, chunk));
                idx
            }
        };

        // safety: index returned above
        unsafe { self.chunks.get_unchecked_mut(idx) }
    }

    pub(crate) fn load_notifications(&self) -> LoadNotifier {
        let send = self.load_notifier.send.clone();
        LoadNotifier {
            send,
            waiters: self.load_notifier.waiters.clone(),
        }
    }

    pub(crate) fn apply_terrain_updates_in_place(
        &mut self,
        updates: impl Iterator<Item = (SlabLocation, impl Iterator<Item = SlabTerrainUpdate<C>>)>,
        mut per_slab: impl FnMut(SlabLocation, OcclusionAffectedNeighbourSlabs),
    ) {
        let mut contiguous_chunks = ContiguousChunkIteratorMut::new(self);

        for (slab_loc, slab_updates) in updates {
            // fetch chunk, reusing the last one if it's the same, as it should be in this sorted iterator
            let chunk = match contiguous_chunks.next(slab_loc.chunk) {
                Some(chunk) => chunk,
                None => {
                    let count = slab_updates.count();
                    debug!("skipping {count} terrain updates for chunk because it's not loaded", count = count; slab_loc.chunk);
                    continue;
                }
            };

            chunk.mark_slab_requested(slab_loc.slab);

            let slab = match chunk.terrain_mut().slab_mut(slab_loc.slab) {
                Some(slab) => slab,
                None => {
                    let count = slab_updates.count();
                    debug!("skipping {count} terrain updates for slab because it's not loaded", count = count; slab_loc);
                    continue;
                }
            };

            let (affected_neighbours, count) = slab.apply_terrain_updates(slab_loc, slab_updates);

            let slab_data = chunk.terrain_mut().slab_data_mut(slab_loc.slab).unwrap(); // just accessed to get terrain
            let new_version = slab_data.mark_modified();

            debug!("applied {count} terrain block updates to slab", count = count; slab_loc, "new_version" => ?new_version);

            per_slab(slab_loc, affected_neighbours);
        }
    }

    /// Panics if chunk doesn't exist. Marks slab as TerrainInWorld
    pub(crate) fn populate_chunk_with_slab(
        &mut self,
        slab: SlabLocation,
        slab_terrain: Option<Slab<C>>,
        vertical_space: Arc<SlabVerticalSpace>,
        occlusion: SparseGrid<BlockOcclusion>,
    ) {
        let chunk = self
            .find_chunk_with_pos_mut(slab.chunk)
            .unwrap_or_else(|| panic!("no such chunk {:?}", slab.chunk));

        // create missing chunks
        let terrain = chunk.terrain_mut();
        terrain.create_slabs_until(slab.slab);

        // TODO unlink all nav from this slab

        trace!("populating slab"; slab.slab);

        // update slab terrain if necessary
        if let Some(terrain) = slab_terrain {
            chunk
                .terrain_mut()
                .replace_slab(slab.slab, SlabData::new(terrain));
        }

        chunk.update_slab_terrain_derived_data(slab.slab, vertical_space, occlusion);

        // notify anyone waiting
        chunk.mark_slab_as_in_world(slab.slab);
    }

    pub fn block<P: Into<WorldPosition>>(&self, pos: P) -> Option<Block<C>> {
        let pos = pos.into();
        self.find_chunk_with_pos(ChunkLocation::from(pos))
            .and_then(|chunk| chunk.terrain().get_block(pos.into()))
    }

    /// Probably incomplete, only useful for lighting
    pub fn block_occlusion_lazy<P: Into<WorldPosition>>(&self, pos: P) -> BlockOcclusion {
        let pos = pos.into();
        self.find_chunk_with_pos(ChunkLocation::from(pos))
            .and_then(|chunk| chunk.terrain().slab_data(pos.2.slab_index()))
            .and_then(|slab| slab.occlusion.get(pos.into()).copied())
            .unwrap_or_default()
    }

    /// Checks against world to find visible faces
    pub fn block_occlusion_complete(&self, pos: impl Into<WorldPosition>) -> BlockOcclusion {
        let pos = pos.into();
        let mut occ = self.block_occlusion_lazy(pos);

        for (n, face) in occ.clone().iter_faces() {
            if n.is_all_solid() {
                continue; // already initialised
            }

            //     // should use RelativeSlabs from occlusion for efficiency
            //     let (slab_dx, pos_in_slab) = slice_block.try_add_intrusive(face.xy_delta());
            //     let new_slab = SlabLocation {
            //         chunk: orig_slab.chunk + (slab_dx[0], slab_dx[1]),
            //         ..orig_slab
            //     };

            let n_pos = if let OcclusionFace::Top = face {
                pos.above()
            } else {
                let (dx, dy) = face.xy_delta();
                pos + (dx as i32, dy as i32, 0)
            };

            // TODO this can definitely be more efficient
            if let Some(b) = self.block(n_pos) {
                if !b.block_type().is_air() {
                    occ.set_face(face, NeighbourOpacity::all_solid());
                }
            }
        }

        occ
    }

    /// Mutates terrain silently to the loader, ensure the loader knows about this
    pub fn damage_block(
        &mut self,
        pos: WorldPosition,
        damage: BlockDurability,
    ) -> Option<BlockDamageResult> {
        self.find_chunk_with_pos_mut(ChunkLocation::from(pos))
            .and_then(|chunk| chunk.terrain_mut().apply_block_damage(pos.into(), damage))
    }

    pub fn nav_graph(&self) -> &WorldGraph {
        &self.nav_graph
    }

    pub fn nav_graph_mut(&mut self) -> &mut WorldGraph {
        &mut self.nav_graph
    }

    #[cfg(test)]
    pub(crate) fn area_graph(&self) -> &AreaGraph {
        &self.area_graph
    }

    pub fn choose_random_accessible_point(
        &self,
        max_attempts: usize,
        requirement: NavRequirement,
        random: &mut dyn RngCore,
    ) -> Option<WorldPoint> {
        (0..max_attempts).find_map(|_| {
            // choose from all global chunks
            let chunk = self.all_chunks().choose(random).unwrap(); // never empty

            // choose random area
            let (a, ai) = chunk
                .iter_areas_with_info()
                .filter(|(a, ai)| ai.fits_requirement(requirement))
                .choose(random)?;

            // take random point in this area
            let (x, y) = ai.random_point(requirement.max_xy, random);

            // TODO new BlockPoint for BlockPosition but floats. this conversion is gross
            let block_pos =
                BlockPosition::new_unchecked(x as BlockCoord, y as BlockCoord, a.slice());
            let world_pos = block_pos.to_world_position(chunk.pos()).floored();

            let point = world_pos + (x.fract(), y.fract(), 0.0);
            Some(point)
        })
    }

    pub fn choose_random_accessible_block(
        &self,
        max_attempts: usize,
        requirement: NavRequirement,
        random: &mut dyn RngCore,
    ) -> Option<WorldPosition> {
        self.choose_random_accessible_point(max_attempts, requirement, random)
            .map(|p| p.floor())
    }

    /// Finds the area for the specific block (air)
    pub fn areav2(&self, pos: WorldPosition) -> Option<WorldAreaV2> {
        let chunk = self.find_chunk_with_pos(pos.into())?;
        let slab_idx = pos.slice().slab_index();
        let slab = chunk.terrain().slab_data(slab_idx)?;

        let slice = pos.slice().to_local();
        slab.nav.iter_nodes().find_map(move |a| {
            if a.slice_idx == slice {
                // bounds check
                let info = chunk
                    .area_info(slab_idx, a)
                    .unwrap_or_else(|| panic!("unknown area {a:?} in chunk {:?}", chunk.pos()));
                if info.contains(pos.into()) {
                    return Some(a.to_chunk_area(slab_idx).to_world_area(chunk.pos()));
                }
            }

            None
        })
    }

    #[deprecated]
    pub fn area<P: Into<WorldPosition>>(&self, pos: P) -> AreaLookup {
        todo!("old nav");
        let block_pos = pos.into();
        let chunk_pos = ChunkLocation::from(block_pos);
        let block = self
            .find_chunk_with_pos(chunk_pos)
            .and_then(|chunk| chunk.terrain().get_block(block_pos.into()));

        let area = match block {
            None => return AreaLookup::BadPosition,
            Some(b) => b.chunk_area(block_pos.slice()),
        };

        match area {
            None => AreaLookup::NoArea,
            Some(area) => AreaLookup::Area(area.into_world_area(chunk_pos)),
        }
    }

    pub fn iterate_blocks(
        &self,
        range: WorldPositionRange,
    ) -> impl Iterator<Item = (Block<C>, WorldPosition)> + '_ {
        range
            .iter_blocks()
            .map(move |pos| (self.block(pos), pos))
            .filter_map(move |(block, pos)| block.map(|b| (b, pos)))
    }

    pub fn filter_blocks_in_range<'a>(
        &'a self,
        range: &WorldPositionRange,
        mut f: impl FnMut(Block<C>, &WorldPosition) -> bool + 'a,
    ) -> impl Iterator<Item = (Block<C>, WorldPosition)> + 'a {
        // TODO benchmark filter_blocks_in_range, then optimize slab and slice lookups

        self.iterate_blocks(range.clone())
            .filter(move |(block, pos)| f(*block, pos))
    }

    /// Filters blocks in the range that 1) pass the blocktype test, and 2) are adjacent to a walkable
    /// accessible block
    pub fn filter_reachable_blocks_in_range<'a>(
        &'a self,
        range: &WorldPositionRange,
        mut f: impl FnMut(C::BlockType) -> bool + 'a,
    ) -> impl Iterator<Item = WorldPosition> + 'a {
        self.filter_blocks_in_range(range, move |b, pos| {
            // check block type
            if !f(b.block_type()) {
                return false;
            }

            // check neighbours for reachability
            // TODO filter_blocks_in_range should pass chunk+slab reference to predicate
            let mut neighbours = WorldNeighbours::new(*pos)
                .chain(once(pos.above())) // above and below too
                .chain(once(pos.below()));

            if neighbours.any(|pos| matches!(self.area(pos), AreaLookup::Area(_))) {
                // at least one neighbour is walkable
                return true;
            }

            false
        })
        .map(|(_, pos)| pos)
    }

    /// Removes slabs that are already loaded and not placeholders.
    /// More efficient when sorted by slab
    pub fn retain_slabs_to_load(&self, slabs: &mut Vec<SlabLocation>) {
        let mut contiguous_chunks = ContiguousChunkIterator::new(self);

        slabs.retain(|slab| {
            match contiguous_chunks.next(slab.chunk) {
                Some(chunk) => chunk.should_slab_be_loaded(slab.slab),
                None => {
                    // chunk is unloaded so its slabs are too
                    true
                }
            }
        });
    }
}

impl Default for LoadNotifier {
    fn default() -> Self {
        let (send, recv) = broadcast::channel(4096);
        Self {
            send,
            waiters: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Drop for ListeningLoadNotifier {
    fn drop(&mut self) {
        // unsubscribe
        self.waiters.fetch_sub(1, Ordering::SeqCst);
    }
}

impl ListeningLoadNotifier {
    pub async fn wait_for_any(&mut self) -> WaitResult {
        loop {
            match self.recv.recv().await {
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("slab notifications are lagging! probably deadlock incoming"; "skipped" => n);
                    break WaitResult::Retry;
                }
                Err(e) => {
                    error!("error waiting for slab notification: {}", e);
                    break WaitResult::Disconnected;
                }
                Ok(_) => break WaitResult::Success,
            }
        }
    }
    /// Ignores messages for other slabs
    pub async fn wait_for_slab(&mut self, slab: SlabLocation) -> WaitResult {
        loop {
            match self.recv.recv().await {
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("slab notifications are lagging! probably deadlock incoming"; "skipped" => n);
                    break WaitResult::Retry;
                }
                Err(e) => {
                    error!("error waiting for slab notification: {}", e);
                    break WaitResult::Disconnected;
                }
                Ok(recvd) if recvd == slab => break WaitResult::Success,
                Ok(_) => { /* keep waiting */ }
            }
        }
    }

    /// Waits for load notifications for all slabs. Ensure no dupes
    pub async fn wait_for_slabs(&mut self, slabs: &[SlabLocation]) -> WaitResult {
        debug_assert_eq!(
            slabs.iter().sorted_unstable().copied().collect_vec(),
            slabs
                .iter()
                .sorted_unstable()
                .copied()
                .dedup()
                .collect_vec(),
            "duplicates detected"
        );

        trace!("waiting for slabs to load: {:?}", slabs);

        let mut success: SmallVec<[bool; 16]> = smallvec![false; 16];
        let mut remaining = slabs.len();

        loop {
            match self.recv.recv().await {
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("slab notifications are lagging! probably deadlock incoming"; "skipped" => n);
                    break WaitResult::Retry;
                }
                Err(e) => {
                    error!("error waiting for slab notification: {}", e);
                    break WaitResult::Disconnected;
                }
                Ok(recvd) => {
                    if let Some(idx) = slabs.iter().position(|s| *s == recvd) {
                        debug_assert!(!success[idx]);
                        success[idx] = true; // TODO what is the point of this then?
                        remaining -= 1;
                        if remaining == 0 {
                            return WaitResult::Success;
                        }
                    }

                    // keep waiting
                    trace!("continue waiting for {remaining}: {:?}", slabs);
                }
            }
        }
    }
}

impl LoadNotifier {
    pub fn start_listening(&self) -> ListeningLoadNotifier {
        self.waiters.fetch_add(1, Ordering::SeqCst);
        ListeningLoadNotifier {
            waiters: self.waiters.clone(),
            recv: self.send.subscribe(),
        }
    }

    pub fn notify(&self, slab: SlabLocation) {
        if self.waiters.load(Ordering::Relaxed) > 0 {
            self.send.send(slab).expect("send channel is full");
        }
    }
}

pub async fn get_or_wait_for_slab_areas<C: WorldContext>(
    notifier: &mut ListeningLoadNotifier,
    world: &WorldRef<C>,
    slab: SlabLocation,
    mut func: impl FnMut(&ChunkArea, &AreaInfo),
) {
    loop {
        {
            let w = world.borrow();
            let chunk = match w.find_chunk_with_pos(slab.chunk) {
                Some(c) => c,
                None => break, // non existent chunk
            };
            match chunk.get_slab_areas_or_wait(slab.slab, |a, ai| func(a, ai)) {
                SlabThingOrWait::Wait => {}
                SlabThingOrWait::Failure | SlabThingOrWait::Ready(_) => break,
            }
        }

        if let WaitResult::Disconnected = notifier.wait_for_slab(slab).await {
            // failure, guess we're shutting down
            break;
        }
    }
}

/// Check if out is populated
pub async fn get_or_collect_slab_areas<C: WorldContext>(
    notifier: &mut ListeningLoadNotifier,
    world: &WorldRef<C>,
    slab: SlabLocation,
    func: impl Fn(&ChunkArea, &AreaInfo) -> Option<(SliceNavArea, SliceAreaIndex)>,
    out: &mut Vec<(SliceNavArea, SliceAreaIndex)>,
) {
    get_or_wait_for_slab_areas(notifier, world, slab, |a, ai| {
        if let Some(nav_area) = func(a, ai) {
            out.push(nav_area);
        }
    })
    .await
}

pub async fn get_or_wait_for_slab_vertical_space<C: WorldContext>(
    notifier: &mut ListeningLoadNotifier,
    world: &WorldRef<C>,
    slab: SlabLocation,
) -> Option<Arc<SlabVerticalSpace>> {
    loop {
        {
            let w = world.borrow();
            let chunk = w.find_chunk_with_pos(slab.chunk)?;
            match chunk.slab_vertical_space_or_wait(slab.slab) {
                SlabThingOrWait::Wait => {}
                SlabThingOrWait::Failure => return None,
                SlabThingOrWait::Ready(x) => return Some(x),
            }
        }

        if let WaitResult::Disconnected = notifier.wait_for_slab(slab).await {
            // failure, guess we're shutting down
            return None;
        }
    }
}

pub async fn get_or_wait_for_slab<C: WorldContext>(
    notifier: &mut ListeningLoadNotifier,
    world: &WorldRef<C>,
    slab: SlabLocation,
) -> Option<Slab<C>> {
    loop {
        {
            let w = world.borrow();
            let chunk = match w.find_chunk_with_pos(slab.chunk) {
                Some(c) => c,
                None => break None, // non existent chunk
            };
            match chunk.get_slab_or_wait(slab.slab) {
                SlabThingOrWait::Wait => {}
                SlabThingOrWait::Failure => break None,
                SlabThingOrWait::Ready(slab) => break Some(slab.clone()),
            }
        }

        if let WaitResult::Disconnected = notifier.wait_for_slab(slab).await {
            // failure, guess we're shutting down
            break None;
        }
    }
}

impl<'a, C: WorldContext> ContiguousChunkIteratorMut<'a, C> {
    pub fn new(world: &'a mut World<C>) -> Self {
        ContiguousChunkIteratorMut {
            world,
            last_chunk: None,
        }
    }

    // Identical to ContiguousChunkIterator just different enough with mut to require too
    // much effort to reduce duplication :( lmao even this comment is the same
    // noinspection DuplicatedCode
    pub fn next(&mut self, chunk: ChunkLocation) -> Option<&mut Chunk<C>> {
        match self.last_chunk.take() {
            Some((last_loc, idx)) if last_loc == chunk => {
                // nice, reuse index of chunk without lookup
                // safety: index returned by last call and no chunks added since because this holds
                // a world reference
                Some(unsafe { self.world.chunks.get_unchecked_mut(idx) })
            }
            _ => {
                // new chunk
                match self.world.find_chunk_index(chunk) {
                    Ok(idx) => {
                        self.last_chunk = Some((chunk, idx));

                        // safety: index returned by find_chunk_index
                        Some(unsafe { self.world.chunks.get_unchecked_mut(idx) })
                    }
                    Err(_) => None,
                }
            }
        }
    }
}

impl<'a, C: WorldContext> ContiguousChunkIterator<'a, C> {
    pub fn new(world: &'a World<C>) -> Self {
        ContiguousChunkIterator {
            world,
            last_chunk: None,
            #[cfg(test)]
            matched_last: false,
        }
    }

    // Identical to ContiguousChunkIteratorMut but just different enough with mut to require too
    // much effort to reduce duplication :( lmao even this comment is the same
    // noinspection DuplicatedCode
    pub fn next(&mut self, chunk: ChunkLocation) -> Option<&Chunk<C>> {
        match self.last_chunk.take() {
            Some((last_loc, idx)) if last_loc == chunk => {
                // nice, reuse index of chunk without lookup
                #[cfg(test)]
                {
                    self.matched_last = true;
                }

                if let Some(idx) = idx {
                    // safety: index returned by last call and no chunks added since because this holds
                    // a world reference
                    Some(unsafe { self.world.chunks.get_unchecked(idx) })
                } else {
                    // does not exist
                    None
                }
            }
            _ => {
                // new chunk

                #[cfg(test)]
                {
                    self.matched_last = false;
                }

                match self.world.find_chunk_index(chunk) {
                    Ok(idx) => {
                        self.last_chunk = Some((chunk, Some(idx)));

                        // safety: index returned by find_chunk_index
                        Some(unsafe { self.world.chunks.get_unchecked(idx) })
                    }
                    Err(_) => {
                        self.last_chunk = Some((chunk, None));
                        None
                    }
                }
            }
        }
    }
}

impl AreaLookup {
    pub fn ok(self) -> Option<WorldArea> {
        match self {
            AreaLookup::Area(area) => Some(area),
            _ => None,
        }
    }
}

/// Helpers to create a world synchronously for tests and benchmarks
pub mod helpers {
    use color::Color;
    use std::sync::Arc;
    use std::time::Duration;

    use misc::Itertools;
    use unit::world::{ChunkLocation, SlabLocation};

    use crate::block::{Block, BlockDurability, BlockOpacity};
    use crate::chunk::slab::SlabGridImpl;
    use crate::chunk::slice::SLICE_SIZE;
    use crate::context::NopGeneratedTerrainSource;
    use crate::loader::{AsyncWorkerPool, MemoryTerrainSource, WorldLoader, WorldTerrainUpdate};
    use crate::{BlockType, Chunk, ChunkBuilder, ChunkDescriptor, WorldContext, WorldRef};

    pub struct DummyWorldContext;

    #[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
    pub enum DummyBlockType {
        Air,
        Dirt,
        Grass,
        Stone,
        Leaves,
        LightGrass,
    }

    impl WorldContext for DummyWorldContext {
        type BlockType = DummyBlockType;
        type GeneratedTerrainSource = NopGeneratedTerrainSource<Self>;
        type GeneratedBlockDetails = ();
        type GeneratedEntityDesc = ();

        fn air_slice() -> &'static [Block<Self>; SLICE_SIZE] {
            todo!()
        }

        fn all_air() -> Arc<SlabGridImpl<Self>> {
            WorldContext::slab_grid_of_all(DummyBlockType::Air)
        }
        fn all_stone() -> Arc<SlabGridImpl<Self>> {
            WorldContext::slab_grid_of_all(DummyBlockType::Stone)
        }

        const PRESET_TYPES: [Self::BlockType; 3] = [
            DummyBlockType::Stone,
            DummyBlockType::Dirt,
            DummyBlockType::Grass,
        ];
    }

    impl BlockType for DummyBlockType {
        const AIR: Self = DummyBlockType::Air;

        fn opacity(&self) -> BlockOpacity {
            if self.is_air() {
                BlockOpacity::Transparent
            } else {
                BlockOpacity::Solid
            }
        }

        fn durability(&self) -> BlockDurability {
            100
        }

        fn is_air(&self) -> bool {
            matches!(self, DummyBlockType::Air)
        }

        fn can_be_walked_on(&self) -> bool {
            !matches!(self, DummyBlockType::Air | DummyBlockType::Leaves)
        }

        fn render_color(&self) -> Color {
            Color::rgb(255, 0, 0)
        }
    }

    pub fn load_single_chunk(chunk: ChunkBuilder<DummyWorldContext>) -> Chunk<DummyWorldContext> {
        let pos = ChunkLocation(0, 0);
        let world = world_from_chunks_blocking(vec![chunk.build(pos)]);
        let mut world = world.borrow_mut();
        let new_chunk = Chunk::empty_with_world(&*world, pos);
        let chunk = world.find_chunk_with_pos_mut(pos).unwrap();
        std::mem::replace(chunk, new_chunk)
    }

    pub fn world_from_chunks_blocking(
        chunks: Vec<ChunkDescriptor<DummyWorldContext>>,
    ) -> WorldRef<DummyWorldContext> {
        loader_from_chunks_blocking(chunks).world()
    }

    pub fn loader_from_chunks_blocking(
        chunks: Vec<ChunkDescriptor<DummyWorldContext>>,
    ) -> WorldLoader<DummyWorldContext> {
        let source = MemoryTerrainSource::from_chunks(chunks.into_iter()).expect("bad chunks");
        load_world(source, AsyncWorkerPool::new(1).unwrap())
    }

    pub fn loader_from_chunks_blocking_with_load_blacklist(
        chunks: Vec<ChunkDescriptor<DummyWorldContext>>,
        blacklist: Vec<SlabLocation>,
    ) -> WorldLoader<DummyWorldContext> {
        let mut source = MemoryTerrainSource::from_chunks(chunks.into_iter()).expect("bad chunks");
        for slab in blacklist {
            source.blacklist_slab_on_initial_load(slab)
        }
        load_world(source, AsyncWorkerPool::new(1).unwrap())
    }

    pub fn test_world_timeout() -> Duration {
        let seconds = std::env::var("NN_TEST_WORLD_TIMEOUT")
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(2);

        Duration::from_secs(seconds)
    }

    pub fn apply_updates(
        loader: &mut WorldLoader<DummyWorldContext>,
        updates: &[WorldTerrainUpdate<DummyWorldContext>],
    ) -> Result<(), String> {
        let mut updates = updates.iter().cloned().collect();
        loader.apply_terrain_updates(&mut updates);
        loader.block_until_all_done(test_world_timeout()).unwrap();

        Ok(())
    }

    pub(crate) fn load_world<C: WorldContext>(
        source: MemoryTerrainSource<C>,
        pool: AsyncWorkerPool,
    ) -> WorldLoader<C> {
        // TODO build area graph in loader
        // let area_graph = AreaGraph::from_chunks(&[]);

        let slabs_to_load = source
            .all_slabs(true)
            .sorted_by(|a, b| a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab)))
            .collect_vec();

        let mut loader = WorldLoader::new(source, pool);
        loader.request_slabs_all(slabs_to_load.into_iter());
        loader.block_until_all_done(test_world_timeout()).unwrap();

        loader
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use std::convert::TryFrom;
    use std::time::Duration;

    use misc::{logging, thread_rng, Itertools, Rng, SeedableRng, StdRng};
    use unit::world::{all_slabs_in_range, SliceBlock, SliceIndex, WorldPoint, CHUNK_SIZE};
    use unit::world::{
        BlockPosition, ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition,
        WorldPositionRange, SLAB_SIZE,
    };

    use crate::chunk::ChunkBuilder;
    use crate::helpers::{DummyBlockType, DummyWorldContext};
    use crate::loader::{AsyncWorkerPool, MemoryTerrainSource, WorldLoader, WorldTerrainUpdate};
    use crate::navigation::EdgeCost;
    use crate::navigationv2::NavRequirement;
    use crate::occlusion::{NeighbourOpacity, VertexOcclusion};
    use crate::presets::from_preset;
    use crate::world::helpers::{
        apply_updates, loader_from_chunks_blocking, world_from_chunks_blocking,
    };
    use crate::world::ContiguousChunkIterator;
    use crate::{presets, BlockType, SearchGoal, World, WorldContext, WorldRef};

    #[test]
    fn world_context() {
        assert!(DummyBlockType::Stone.can_be_walked_on());
        assert!(DummyBlockType::Stone.opacity().solid());
        assert!(!DummyBlockType::Stone.is_air());

        assert!(!DummyBlockType::Air.can_be_walked_on());
        assert!(DummyBlockType::Air.opacity().transparent());
        assert!(DummyBlockType::Air.is_air());
    }

    #[test]
    fn world_path_single_block_in_y_direction() {
        let w = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(1, DummyBlockType::Grass)
            .build((0, 0))])
        .into_inner();

        let path = w
            .find_path((2, 2, 2), (2, 3, 2))
            .expect("path should succeed");

        assert_eq!(path.path().len(), 1);
    }

    #[test]
    fn world_path_single_block_within_radius_of_one() {
        let w = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(1, DummyBlockType::Grass)
            .build((0, 0))])
        .into_inner();

        let path = w
            .find_path_with_goal((2, 2, 2).into(), (4, 3, 2).into(), SearchGoal::Nearby(5)) // immediately satisfied
            .expect("path should succeed");

        assert_eq!(path.path().len(), 1);
    }

    #[test]
    fn accessible_block_in_column() {
        let w = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(6, DummyBlockType::Grass) // lower slice
            .fill_slice(9, DummyBlockType::Grass) // higher slice blocks it...
            .set_block((10, 10, 9), DummyBlockType::Air) // ...with a hole here
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
    fn accessible_block_unwalkable_types() {
        let w = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(6, DummyBlockType::Leaves) // full of unwalkable block
            .set_block((4, 4, 6), DummyBlockType::Grass) // single block of sanctuary
            .build((0, 0))])
        .into_inner();

        //
        assert_eq!(
            w.find_accessible_block_in_column(4, 4),
            Some((4, 4, 7).into())
        );

        // all leaves
        assert!(w.find_accessible_block_in_column(3, 3).is_none());
        assert!(w.find_accessible_block_in_column(4, 3).is_none());
        assert!(w.find_accessible_block_in_column(0, 0).is_none());
    }

    #[test]
    fn world_path_within_area() {
        let world = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_slice(2, DummyBlockType::Stone)
            .set_block((0, 0, 3), DummyBlockType::Grass)
            .set_block((8, 8, 3), DummyBlockType::Grass)
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
        // cross chunks
        let world = world_from_chunks_blocking(vec![
            ChunkBuilder::new()
                .fill_slice(4, DummyBlockType::Grass)
                .build((3, 5)),
            ChunkBuilder::new()
                .fill_slice(4, DummyBlockType::Grass)
                .build((4, 5)),
            ChunkBuilder::new()
                .fill_slice(5, DummyBlockType::Grass)
                .build((5, 5)),
            ChunkBuilder::new()
                .fill_slice(6, DummyBlockType::Grass)
                .build((6, 5)),
            ChunkBuilder::new()
                .fill_slice(6, DummyBlockType::Grass)
                .build((6, 4)),
            ChunkBuilder::new()
                .fill_slice(6, DummyBlockType::Grass)
                .build((6, 3)),
            ChunkBuilder::new().build((0, 0)),
        ])
        .into_inner();

        let from = BlockPosition::new_unchecked(0, 2, 5.into()).to_world_position((3, 5));
        let to = BlockPosition::new_unchecked(5, 8, 7.into()).to_world_position((6, 3));

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
        let world = world_from_chunks_blocking(presets::ring());

        let src = WorldPoint::new_unchecked(8.5, 25.5, 302.0);
        let dst = WorldPoint::new_unchecked(-6.5, 24.5, 301.0);

        let _path = World::find_path_now(
            world.clone(),
            src,
            dst,
            NavRequirement {
                height: 2,
                max_xy: 1,
            },
        )
        .expect("path should succeed");
    }

    #[test]
    fn same_area() {
        let world = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_range((2, 2, 2), (6, 6, 2), |_| DummyBlockType::Stone)
            .build((0, 0))]);

        let src = WorldPoint::new_unchecked(2.0, 2.0, 3.0);
        let dst = WorldPoint::new_unchecked(4.0, 4.0, 3.0);

        let path = World::find_path_now(
            world.clone(),
            src,
            dst,
            NavRequirement {
                height: 2,
                max_xy: 1,
            },
        )
        .expect("path should succeed");
        assert_eq!(path.iter_areas().count(), 0);
    }

    #[test]
    fn adjacent_area() {
        /*
           X X
               X X

           0 0 1 1
           X X X X
        */
        let world = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_range((2, 2, 2), (5, 2, 2), |_| DummyBlockType::Stone)
            .fill_range((2, 2, 6), (3, 2, 6), |_| DummyBlockType::Stone)
            .fill_range((4, 2, 5), (5, 2, 5), |_| DummyBlockType::Stone)
            .build((0, 0))]);

        let src = WorldPoint::new_unchecked(2.0, 2.0, 3.0);
        let dst = WorldPoint::new_unchecked(5.0, 2.0, 3.0);

        let path = World::find_path_now(
            world.clone(),
            src,
            dst,
            NavRequirement {
                height: 2,
                max_xy: 1,
            },
        )
        .expect("path should succeed");
        assert_ne!(path.iter_areas().count(), 0);
        assert_eq!(path.route().count(), 1);
    }

    #[test]
    fn world_path_adjacent_goal() {
        let world = world_from_chunks_blocking(vec![ChunkBuilder::new()
            .fill_range((2, 2, 2), (6, 2, 2), |_| DummyBlockType::Stone)
            .build((0, 0))])
        .into_inner();

        let path = world
            .find_path_with_goal((2, 2, 3).into(), (6, 2, 3).into(), SearchGoal::Adjacent)
            .expect("path should succeed");

        // target should be the adjacent block, not the given target
        assert_eq!(path.target(), (5, 2, 3).into());
        assert_eq!(path.path().len(), 3);
    }

    #[test]
    fn find_chunk() {
        let world = world_from_chunks_blocking(vec![
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

        assert!(world.find_chunk_with_pos(ChunkLocation(10, 10)).is_none());
    }

    #[test]
    fn terrain_updates_applied() {
        // logging::for_tests();

        let chunks = vec![
            ChunkBuilder::new()
                .fill_range((0, 0, 0), (5, 5, 5), |_| DummyBlockType::Stone)
                .set_block((10, 10, 200), DummyBlockType::Air) // to add the slabs inbetween
                .set_block((CHUNK_SIZE.as_i32() - 1, 5, 5), DummyBlockType::Stone) // occludes the block below
                .build((0, 0)),
            ChunkBuilder::new()
                .set_block((0, 5, 4), DummyBlockType::Stone) // occluded by the block above
                .build((1, 0)),
        ];

        let updates = vec![
            WorldTerrainUpdate::new(
                WorldPositionRange::with_single((1, 1, 1)),
                DummyBlockType::Grass,
            ),
            WorldTerrainUpdate::new(
                WorldPositionRange::with_single((10, 10, 200)),
                DummyBlockType::LightGrass,
            ),
            WorldTerrainUpdate::new(
                WorldPositionRange::with_single((CHUNK_SIZE.as_i32() - 1, 5, 5)),
                DummyBlockType::Air,
            ),
        ];
        let mut loader = loader_from_chunks_blocking(chunks);
        let world = loader.world();

        {
            // pre update checks
            let w = world.borrow();
            assert_eq!(
                w.block((1, 1, 1)).unwrap().block_type(), // range filled with stone
                DummyBlockType::Stone
            );
            assert_eq!(
                w.block((10, 10, 200)).unwrap().block_type(), // high up block empty
                DummyBlockType::Air
            );
            assert_eq!(
                w.block_occlusion_lazy((CHUNK_SIZE.as_i32(), 5, 4))
                    .top_corner(3), // occluded by other chunk
                VertexOcclusion::Mildly
            );
        }

        // apply updates
        apply_updates(&mut loader, &updates).expect("updates failed");

        {
            // post update checks
            let w = world.borrow();
            assert_eq!(
                w.block((1, 1, 1)).unwrap().block_type(), // stone updated
                DummyBlockType::Grass
            );
            assert_eq!(
                w.block((10, 10, 200)).unwrap().block_type(), // air updated
                DummyBlockType::LightGrass
            );
            assert_eq!(
                w.block((10, 10, 199)).unwrap().block_type(), // uninitialized air untouched
                DummyBlockType::Air
            );
            assert_eq!(
                w.block((2, 2, 2)).unwrap().block_type(), // stone untouched
                DummyBlockType::Stone
            );
            assert_eq!(
                w.block_occlusion_lazy((CHUNK_SIZE.as_i32(), 5, 4))
                    .top_corner(3), // no longer occluded by other chunk
                VertexOcclusion::NotAtAll
            );
        }
    }

    #[test]
    fn load_world_blocking() {
        let world = world_from_chunks_blocking(vec![ChunkBuilder::new().build((0, 0))]);
        let mut w = world.borrow_mut();

        assert_eq!(
            w.block((0, 0, 0)).unwrap().block_type(),
            DummyBlockType::Air
        );

        *w.find_chunk_with_pos_mut(ChunkLocation(0, 0))
            .unwrap()
            .terrain_mut()
            .slice_mut(0)
            .unwrap()[(0, 0)]
            .block_type_mut() = DummyBlockType::Stone;

        assert_eq!(
            w.block((0, 0, 0)).unwrap().block_type(),
            DummyBlockType::Stone
        );
    }

    #[ignore]
    #[test]
    fn random_terrain_updates_stresser() {
        // const CHUNK_RADIUS: i32 = 8;
        const TERRAIN_HEIGHT: f64 = 400.0;

        const UPDATE_SETS: usize = 100;
        const UPDATE_REPS: usize = 10;

        let pool = AsyncWorkerPool::new(4).unwrap();
        // TODO make stresser use generated terrain again
        // let source =
        //     GeneratedTerrainSource::new(None, CHUNK_RADIUS as u32, TERRAIN_HEIGHT).unwrap();
        let source = from_preset::<DummyWorldContext>("multichunkwonder", &mut thread_rng());
        let (min, max) = source.world_bounds();
        let mut loader = WorldLoader::new(source, pool);

        let max_slab = (TERRAIN_HEIGHT as f32 / SLAB_SIZE.as_f32()).ceil() as i32 + 1;
        let (all_slabs, count) = all_slabs_in_range(
            SlabLocation::new(-max_slab, min),
            SlabLocation::new(max_slab, max),
        );
        loader.request_slabs(all_slabs);

        assert!(loader.block_until_all_done(Duration::from_secs(60)).is_ok());

        let world = loader.world();

        let mut randy = StdRng::from_entropy();
        for i in 0..UPDATE_SETS {
            let updates = (0..UPDATE_REPS)
                .map(|_| {
                    let w = world.borrow();
                    let pos = w
                        .choose_random_accessible_block(1000, NavRequirement::MIN, &mut randy)
                        .expect("ran out of walkable blocks");
                    let blocktype = if randy.gen_bool(0.5) {
                        DummyBlockType::Stone
                    } else {
                        DummyBlockType::Air
                    };

                    let range = if randy.gen_bool(0.2) {
                        WorldPositionRange::with_single(pos)
                    } else {
                        let w = randy.gen_range(1, 10);
                        let h = randy.gen_range(1, 10);
                        let d = randy.gen_range(1, 10);

                        WorldPositionRange::with_inclusive_range(pos, pos + (w, h, d))
                    };
                    WorldTerrainUpdate::new(range, blocktype)
                })
                .collect_vec();

            eprintln!(
                "STRESSER ({}/{}) applying {} updates...\n{:#?}",
                i, UPDATE_SETS, UPDATE_REPS, updates
            );
            apply_updates(&mut loader, updates.as_slice()).expect("updates failed");
        }
    }

    #[test]
    fn filter_blocks() {
        let chunks = vec![ChunkBuilder::new()
            .set_block((5, 6, 4), DummyBlockType::Stone)
            .set_block((5, 5, 5), DummyBlockType::LightGrass)
            .set_block((5, 5, 8), DummyBlockType::Grass)
            .build((0, 0))];

        let loader = loader_from_chunks_blocking(chunks);
        let world = loader.world();
        let w = world.borrow();

        let range = WorldPositionRange::with_inclusive_range((4, 7, 4), (6, 3, 9));
        let filtered = w
            .filter_blocks_in_range(&range, |b, pos| {
                b.block_type() != DummyBlockType::Air && pos.slice().slice() < 6
            })
            .sorted_by_key(|(_, pos)| pos.slice())
            .collect_vec();

        assert_eq!(filtered.len(), 2);

        let (block, pos) = filtered[0];
        assert_eq!(block.block_type(), DummyBlockType::Stone);
        assert_eq!(pos, (5, 6, 4).into());

        let (block, pos) = filtered[1];
        assert_eq!(block.block_type(), DummyBlockType::LightGrass);
        assert_eq!(pos, (5, 5, 5).into());
    }

    #[test]
    fn filter_reachable_blocks() {
        let chunks = vec![ChunkBuilder::new()
            .fill_range((0, 0, 0), (8, 8, 3), |_| DummyBlockType::Stone) // floor
            .fill_range((5, 0, 4), (8, 8, 8), |_| DummyBlockType::Stone) // big wall
            .fill_range((0, 0, 5), (8, 8, 8), |_| DummyBlockType::Stone) // ceiling
            .build((0, 0))];

        let loader = loader_from_chunks_blocking(chunks);
        let world = loader.world();
        let w = world.borrow();

        let is_reachable = |xyz: (i32, i32, i32)| {
            let range = WorldPositionRange::with_single(xyz);
            w.filter_reachable_blocks_in_range(&range, |bt| !bt.is_air())
                .count()
                == 1
        };

        // block in the floor can be stood on
        assert!(is_reachable((1, 1, 3)));

        // ... under the floor cannot
        assert!(!is_reachable((1, 1, 2)));

        // ... under the world is technically visible but not reachable
        assert!(!is_reachable((1, 1, 0)));

        // block exposed in the wall on ground level can be accessed from the side
        assert!(is_reachable((5, 0, 4)));

        // ... not the when its too high though
        assert!(!is_reachable((5, 0, 5)));

        // block exposed in the ceiling can be reached from below
        assert!(is_reachable((1, 1, 5)));
    }

    // #[test]
    // fn associated_block_data() {
    //     impl WorldContext for u32 {
    //         type AssociatedBlockData = u32;
    //     }
    //
    //     let source =
    //         MemoryTerrainSource::from_chunks(vec![ChunkBuilder::new().build((0, 0))].into_iter())
    //             .unwrap();
    //     let loader: WorldLoader<u32> = load_world(source, AsyncWorkerPool::new_blocking().unwrap());
    //     let worldref = loader.world();
    //     let mut world = worldref.borrow_mut();
    //
    //     let pos = WorldPosition::from((0, 0, 0));
    //
    //     assert!(world.associated_block_data(pos).is_none());
    //
    //     assert!(world.set_associated_block_data(pos, 50).is_none());
    //     assert_eq!(world.set_associated_block_data(pos, 100), Some(50));
    //
    //     assert_eq!(world.associated_block_data(pos).copied(), Some(100));
    // }

    #[test]
    fn contiguous_chunk_iter() {
        let chunks = vec![
            ChunkBuilder::new().build((0, 0)),
            ChunkBuilder::new().build((1, 0)),
        ];
        let world = world_from_chunks_blocking(chunks);
        let world = world.borrow();
        let mut iter = ContiguousChunkIterator::new(&*world);
        let mut chunk;

        chunk = iter.next(ChunkLocation(0, 0));
        assert!(chunk.is_some());

        // same as before
        chunk = iter.next(ChunkLocation(0, 0));
        assert!(chunk.is_some());
        assert!(iter.matched_last);

        // changed
        chunk = iter.next(ChunkLocation(1, 0));
        assert!(chunk.is_some());
        assert!(!iter.matched_last);

        // same
        chunk = iter.next(ChunkLocation(1, 0));
        assert!(chunk.is_some());
        assert!(iter.matched_last);

        // bad
        chunk = iter.next(ChunkLocation(5, 0));
        assert!(chunk.is_none());
        assert!(!iter.matched_last);

        // still bad but matched
        chunk = iter.next(ChunkLocation(5, 0));
        assert!(chunk.is_none());
        assert!(iter.matched_last);

        // another bad
        chunk = iter.next(ChunkLocation(6, 0));
        assert!(chunk.is_none());
        assert!(!iter.matched_last);
    }
}
