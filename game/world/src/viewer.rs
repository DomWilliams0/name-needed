use common::*;

use crate::mesh::BaseVertex;
use crate::{all_slabs_in_range, mesh, InnerWorldRef, WorldRef};
use std::collections::HashSet;
use std::ops::{Add, Range};
use unit::world::{ChunkLocation, GlobalSliceIndex, SlabLocation};

#[derive(Clone)]
pub struct WorldViewer<D> {
    world: WorldRef<D>,
    view_range: SliceRange,
    chunk_range: (ChunkLocation, ChunkLocation),
    clean_chunks: HashSet<ChunkLocation>,
    requested_slabs: Vec<SlabLocation>,
}

#[derive(Debug, Clone, Error)]
pub enum WorldViewerError {
    #[error("Failed to position viewer, no block found at ({0}, {1})")]
    InvalidStartColumn(i32, i32),

    #[error("Bad viewer range: {0}")]
    InvalidRange(SliceRange),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SliceRange(GlobalSliceIndex, GlobalSliceIndex);

pub struct RequestedSlabs<'a>(&'a mut Vec<SlabLocation>);

impl SliceRange {
    fn new(top: GlobalSliceIndex, size: i32) -> Self {
        assert!(size > 0); // TODO Result
        Self::from_bounds_unchecked(top - size, top)
    }

    pub fn from_bounds<F: Into<GlobalSliceIndex>, T: Into<GlobalSliceIndex>>(
        from: F,
        to: T,
    ) -> Option<Self> {
        let from = from.into();
        let to = to.into();

        if from < to {
            Some(Self::from_bounds_unchecked(from, to))
        } else {
            None
        }
    }

    pub fn from_bounds_unchecked<F: Into<GlobalSliceIndex>, T: Into<GlobalSliceIndex>>(
        from: F,
        to: T,
    ) -> Self {
        let from = from.into();
        let to = to.into();
        debug_assert!(from < to);
        Self(from, to)
    }

    pub fn all() -> Self {
        Self::from_bounds_unchecked(GlobalSliceIndex::bottom(), GlobalSliceIndex::top())
    }

    pub fn contains<S: Into<GlobalSliceIndex>>(self, slice: S) -> bool {
        let slice = slice.into();
        self.as_range().contains(&slice.slice())
    }

    pub const fn bottom(self) -> GlobalSliceIndex {
        self.0
    }
    pub const fn top(self) -> GlobalSliceIndex {
        self.1
    }

    pub fn as_range(self) -> Range<i32> {
        self.0.slice()..self.1.slice()
    }

    pub fn size(self) -> u32 {
        (self.1.slice() - self.0.slice()) as u32
    }
}

impl Display for SliceRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "[{} => {}]", self.0.slice(), self.1.slice())
    }
}

impl Add<i32> for SliceRange {
    type Output = SliceRange;

    fn add(self, rhs: i32) -> Self::Output {
        SliceRange(self.0 + rhs, self.1 + rhs)
    }
}

impl<D> WorldViewer<D> {
    pub fn with_world(world: WorldRef<D>) -> Result<Self, WorldViewerError> {
        let world_borrowed = world.borrow();

        // TODO determine viewer start pos from world/randomly e.g. ground level
        let start_pos = (0, 0);
        let start = world_borrowed
            .find_accessible_block_in_column(start_pos.0, start_pos.1)
            .ok_or(WorldViewerError::InvalidStartColumn(
                start_pos.0,
                start_pos.1,
            ))?
            .2;

        let world_bounds = world_borrowed.slice_bounds().unwrap(); // world has at least 1 slice as above
        drop(world_borrowed);

        // TODO intelligently choose an initial view range
        let range_size = config::get().display.initial_view_range;
        let half_range = (range_size / 2) as i32;
        let view_range = SliceRange::from_bounds(
            (start - half_range).max(world_bounds.bottom()),
            (start + half_range).min(world_bounds.top()),
        )
        .ok_or(WorldViewerError::InvalidRange(world_bounds))?;

        Ok(Self {
            world,
            view_range,
            // TODO receive initial chunk+slab range from engine
            chunk_range: (ChunkLocation(-1, -1), ChunkLocation(1, 1)),
            clean_chunks: HashSet::with_capacity(128),
            requested_slabs: Vec::with_capacity(128),
        })
    }

    pub fn regenerate_dirty_chunk_meshes<F: FnMut(ChunkLocation, Vec<V>), V: BaseVertex>(
        &mut self,
        mut f: F,
    ) {
        let range = self.terrain_range();
        let world = self.world.borrow();
        self.visible_chunks()
            .filter(|c| self.is_chunk_dirty(c))
            .filter_map(|c| world.find_chunk_with_pos(c))
            .for_each(|chunk| {
                // TODO do mesh generation on a worker thread
                let mesh = mesh::make_simple_render_mesh(chunk, range);
                trace!("chunk mesh has {count} vertices", count = mesh.len(); chunk.pos());
                f(chunk.pos(), mesh);
            });

        drop(world);

        self.clean_chunks.extend(self.visible_chunks());
    }

    fn invalidate_meshes(&mut self) {
        // TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly
        self.clean_chunks.clear();
    }

    fn update_range(&mut self, new_range: SliceRange, past_participle: &str, infinitive: &str) {
        // TODO cache world slice_bounds()
        let bounds = self.world.borrow().slice_bounds();
        if let Some(slice_bounds) = bounds {
            if slice_bounds.contains(new_range.0) && slice_bounds.contains(new_range.1) {
                self.view_range = new_range;
                self.invalidate_meshes();
                info!(
                    "{} view range",
                    past_participle; "range" => %self.view_range,
                );
            } else {
                info!(
                    "cannot {} view range",
                    infinitive; "range" => %self.view_range, "world_range" => %slice_bounds,
                );
            }
        }
    }

    // TODO which direction to stretch view range in? automatically determine or player input?
    pub fn stretch_by(&mut self, delta: i32) {
        let bottom = self.view_range.bottom();
        let new_top = self.view_range.top() + delta;
        if let Some(new_range) = SliceRange::from_bounds(bottom, new_top) {
            self.update_range(new_range, "stretched", "stretch")
        }
    }

    pub fn move_by(&mut self, delta: i32) {
        self.update_range(self.view_range + delta, "moved", "move");
    }

    pub fn move_by_multiple(&mut self, delta: i32) {
        let size = self.view_range.size();
        self.move_by(delta * size as i32);
    }

    pub fn visible_chunks(&self) -> impl Iterator<Item = ChunkLocation> {
        let (min, max) = self.chunk_range;
        let xrange = min.0 - 1..=max.0 + 1; // +1 for little buffer
        let yrange = min.1 - 1..=max.1 + 1;

        xrange
            .cartesian_product(yrange)
            .map(|(x, y)| ChunkLocation(x, y))
    }

    pub fn world(&self) -> InnerWorldRef<D> {
        self.world.borrow()
    }

    /// Slice range for terrain rendering
    pub fn terrain_range(&self) -> SliceRange {
        self.view_range
    }

    /// Slice range for entity rendering: 1 above the terrain. This means we will
    /// always be able to see entities walking above the bottom slice of terrain (never floating),
    /// and on top of the highest slice.
    pub fn entity_range(&self) -> SliceRange {
        self.view_range + 1
    }

    pub fn set_chunk_bounds(&mut self, range: (ChunkLocation, ChunkLocation)) {
        let prev_range = self.chunk_range;

        debug_assert!(range.0 <= range.1);
        self.chunk_range = range;

        if prev_range != range {
            // new chunks are visible and should be loaded
            // TODO submit only the new chunks in range
            let (from_chunk, to_chunk) = range;
            let slice_range = self.terrain_range();
            let from = SlabLocation::new(slice_range.bottom().slab_index(), from_chunk);
            let to = SlabLocation::new(slice_range.top().slab_index(), to_chunk);

            let (slab_range, slab_count) = all_slabs_in_range(from, to);
            self.requested_slabs.extend(slab_range);
            trace!("camera movement requested loading of {count} slabs", count = slab_count; "from" => from, "to" => to);
        }
    }

    fn is_chunk_dirty(&self, chunk: &ChunkLocation) -> bool {
        !self.clean_chunks.contains(chunk)
    }

    pub fn mark_dirty(&mut self, chunk: ChunkLocation) {
        self.clean_chunks.remove(&chunk);
    }

    /// Returns deduped and sorted by chunk+slab, inner vec is cleared on ret value drop
    pub fn requested_slabs(
        &mut self,
        extras: impl Iterator<Item = SlabLocation>,
    ) -> RequestedSlabs {
        // include extra requested slabs
        self.requested_slabs.extend(extras);

        let len_before = self.requested_slabs.len();

        // sort by chunk and slab and remove duplicates
        self.requested_slabs
            .sort_unstable_by(|a, b| a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab)));
        self.requested_slabs.dedup();

        // filter down any already loaded slabs
        let world = self.world.borrow();
        world.retain_unloaded_slabs(&mut self.requested_slabs);
        drop(world);

        if len_before > 0 {
            debug!(
                "filtered {unfiltered} slab requests down to {filtered}",
                unfiltered = len_before,
                filtered = self.requested_slabs.len()
            );
        }

        RequestedSlabs(&mut self.requested_slabs)
    }
}

impl Drop for RequestedSlabs<'_> {
    fn drop(&mut self) {
        self.0.clear();
    }
}

impl AsRef<[SlabLocation]> for RequestedSlabs<'_> {
    fn as_ref(&self) -> &[SlabLocation] {
        &self.0
    }
}
