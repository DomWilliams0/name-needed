use misc::*;

use crate::mesh::BaseVertex;
use crate::{mesh, InnerWorldRef, WorldContext, WorldRef};
use futures::FutureExt;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::ops::{Add, RangeInclusive};
use unit::world::{
    all_slabs_in_range, ChunkLocation, GlobalSliceIndex, SlabIndex, SlabLocation, SliceIndex,
    WorldPosition,
};

#[derive(Clone)]
pub struct WorldViewer<C: WorldContext> {
    world: WorldRef<C>,
    view_range: SliceRange,
    chunk_range: (ChunkLocation, ChunkLocation),
    clean_slabs: HashSet<SlabLocation>,
    requested_slabs: Vec<SlabLocation>,

    /// Any slabs in this set will not be requested again
    all_requested_slabs: HashSet<SlabLocation>,
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

    #[inline]
    pub fn contains<S: Into<GlobalSliceIndex>>(self, slice: S) -> bool {
        let slice = slice.into();
        self.as_range().contains(&slice.slice())
    }

    pub fn intersects(self, (from, to): (GlobalSliceIndex, GlobalSliceIndex)) -> bool {
        self.1 >= from && to >= self.0
    }

    pub const fn bottom(self) -> GlobalSliceIndex {
        self.0
    }
    pub const fn top(self) -> GlobalSliceIndex {
        self.1
    }

    #[inline]
    pub fn as_range(self) -> RangeInclusive<i32> {
        self.0.slice()..=self.1.slice()
    }

    pub fn slabs(self) -> impl Iterator<Item = SlabIndex> {
        self.as_range()
            .map(|slice| GlobalSliceIndex::new(slice).slab_index())
            .dedup()
    }

    pub fn size(self) -> u32 {
        (self.1.slice() - self.0.slice()) as u32
    }
}

impl Display for SliceRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.0.slice(), self.1.slice())
    }
}

impl Add<i32> for SliceRange {
    type Output = SliceRange;

    fn add(self, rhs: i32) -> Self::Output {
        SliceRange(self.0 + rhs, self.1 + rhs)
    }
}

impl From<SliceRange> for (GlobalSliceIndex, GlobalSliceIndex) {
    fn from(range: SliceRange) -> Self {
        (range.0, range.1)
    }
}

impl<C: WorldContext> WorldViewer<C> {
    pub fn with_world(
        world: WorldRef<C>,
        initial_block: WorldPosition,
        initial_view_size: u16,
    ) -> Result<Self, WorldViewerError> {
        let world_bounds = {
            let w = world.borrow();
            w.slice_bounds().expect("world should not be empty")
        };

        let view_range = {
            let half_range = (initial_view_size / 2) as i32;
            let centre_slice = initial_block.slice();
            SliceRange::from_bounds(
                (centre_slice - half_range).max(world_bounds.bottom()),
                (centre_slice + half_range).min(world_bounds.top()),
            )
            .ok_or(WorldViewerError::InvalidRange(world_bounds))?
        };

        info!("positioning world viewer at {:?}", view_range);

        let initial_chunk = ChunkLocation::from(initial_block);
        Ok(Self {
            world,
            view_range,
            chunk_range: (initial_chunk, initial_chunk), // TODO is this ok?
            clean_slabs: HashSet::with_capacity(128),
            requested_slabs: Vec::with_capacity(128),
            all_requested_slabs: HashSet::with_capacity(512),
        })
    }

    pub fn regenerate_dirty_chunk_meshes<F: FnMut(ChunkLocation, Vec<V>), V: BaseVertex>(
        &mut self,
        mut f: F,
    ) {
        let _span = misc::tracy_client::span!();

        let range = self.terrain_range();
        let world = self.world.borrow(); // TODO wait up to a time before giving up

        for dirty_chunk in self
            .visible_slabs(range)
            .filter_map(|slab| {
                if self.is_slab_dirty(&slab) {
                    Some(slab.chunk)
                } else {
                    None
                }
            })
            .dedup()
            .filter_map(|chunk| world.find_chunk_with_pos(chunk))
        {
            // TODO do mesh generation on a worker thread? or just do this bit in a parallel iter
            let mesh = mesh::make_simple_render_mesh(dirty_chunk, range);
            debug!("chunk mesh has {count} vertices", count = mesh.len(); dirty_chunk.pos());
            f(dirty_chunk.pos(), mesh);
        }

        drop(world);

        self.clean_slabs.extend(self.visible_slabs(range));
    }

    fn invalidate_meshes(&mut self) {
        // TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly
        self.clean_slabs.clear();
    }

    fn update_range(&mut self, new_range: SliceRange, wat: &str) {
        // TODO limit to loaded slab bounds if camera is not discovering
        self.view_range = new_range;
        self.invalidate_meshes();
        info!(
            "{} view range",
            wat; "range" => %self.view_range,
        );

        // request new slabs
        // TODO only request slabs that are newly visible
        let (bottom_slab, top_slab) = (
            new_range.bottom().slab_index(),
            new_range.top().slab_index(),
        );
        let (bottom_chunk, top_chunk) = self.chunk_range;

        let from = SlabLocation::new(bottom_slab, bottom_chunk);
        let to = SlabLocation::new(top_slab, top_chunk);
        let (slabs, slab_count) = all_slabs_in_range(from, to);
        self.requested_slabs.extend(slabs);
        trace!("vertical camera movement requested loading of {count} slabs", count = slab_count; "from" => from, "to" => to);
    }

    // TODO which direction to stretch view range in? automatically determine or player input?
    pub fn stretch_by(&mut self, delta: i32) {
        let bottom = self.view_range.bottom();
        let new_top = self.view_range.top() + delta;
        if let Some(new_range) = SliceRange::from_bounds(bottom, new_top) {
            self.update_range(new_range, "stretched")
        }
    }

    pub fn move_by(&mut self, delta: i32) {
        self.update_range(self.view_range + delta, "moved");
    }

    pub fn move_by_multiple(&mut self, delta: i32) {
        let size = self.view_range.size();
        self.move_by(delta * size as i32);
    }

    pub fn visible_chunks(&self) -> impl Iterator<Item = ChunkLocation> {
        let (min, max) = self.chunk_range;
        let xrange = min.0 - 1..=max.0;
        let yrange = min.1 - 1..=max.1;

        xrange
            .cartesian_product(yrange)
            .map(|(x, y)| ChunkLocation(x, y))
    }

    pub fn visible_slabs(&self, range: SliceRange) -> impl Iterator<Item = SlabLocation> {
        let (bottom_slab, top_slab) = (range.bottom().slab_index(), range.top().slab_index());
        let (bottom_chunk, top_chunk) = self.chunk_range;

        let from = SlabLocation::new(bottom_slab, bottom_chunk);
        let to = SlabLocation::new(top_slab, top_chunk);
        let (slabs, _) = all_slabs_in_range(from, to);
        slabs
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

    pub fn chunk_range(&self) -> (ChunkLocation, ChunkLocation) {
        self.chunk_range
    }

    fn is_slab_dirty(&self, slab: &SlabLocation) -> bool {
        !self.clean_slabs.contains(slab)
    }

    pub fn mark_dirty(&mut self, slab: SlabLocation) {
        self.clean_slabs.remove(&slab);
    }

    /// Returns deduped and unrequested, sorted by chunk+slab. Call `consume` when requested to clear out the
    /// requested ones.
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
        self.requested_slabs
            .retain(|s| !self.all_requested_slabs.contains(s));

        if len_before > 0 {
            debug!(
                "filtered {unfiltered} slab requests down to {filtered}",
                unfiltered = len_before,
                filtered = self.requested_slabs.len()
            );
        }

        if !self.requested_slabs.is_empty() {
            trace!("slab requests"; "slabs" => ?self.requested_slabs);

            self.all_requested_slabs
                .extend(self.requested_slabs.iter().copied());
        }

        RequestedSlabs(&mut self.requested_slabs)
    }
}

impl RequestedSlabs<'_> {
    pub fn consume(self, n: usize) {
        let _ = self.0.drain(0..n);
    }
}

impl AsRef<[SlabLocation]> for RequestedSlabs<'_> {
    fn as_ref(&self) -> &[SlabLocation] {
        self.0
    }
}

impl RequestedSlabs<'_> {
    pub fn filter(&mut self, pred: impl Fn(SlabLocation) -> bool) {
        self.0.retain(|s| pred(*s));
    }
}
