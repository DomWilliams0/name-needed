use std::fmt::{Display, Error, Formatter};
use std::ops::{Add, Range};

use common::*;

use crate::mesh::BaseVertex;
use crate::{mesh, InnerWorldRef, WorldRef};
use std::collections::HashSet;
use unit::world::{ChunkPosition, GlobalSliceIndex};

#[derive(Clone)]
pub struct WorldViewer {
    world: WorldRef,
    view_range: SliceRange,
    chunk_range: (ChunkPosition, ChunkPosition),
    clean_chunks: HashSet<ChunkPosition>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SliceRange(GlobalSliceIndex, GlobalSliceIndex);

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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "[{} => {}]", self.0.slice(), self.1.slice())
    }
}

impl Add<i32> for SliceRange {
    type Output = SliceRange;

    fn add(self, rhs: i32) -> Self::Output {
        SliceRange(self.0 + rhs, self.1 + rhs)
    }
}

impl WorldViewer {
    pub fn from_world(world: WorldRef) -> Self {
        let world_borrowed = world.borrow();

        // TODO return Result from from_world
        let start = world_borrowed
            .find_accessible_block_in_column(0, 0)
            .expect("there's a hole at 0,0??")
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
        .expect("bad view range");
        Self {
            world,
            view_range,
            chunk_range: (ChunkPosition(-1, -1), ChunkPosition(1, 1)),
            clean_chunks: HashSet::with_capacity(128),
        }
    }

    pub fn regenerate_dirty_chunk_meshes<F: FnMut(ChunkPosition, Vec<V>), V: BaseVertex>(
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
                trace!("chunk mesh {:?} has {} vertices", chunk.pos(), mesh.len());
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
                    "{} view range, new range is {}",
                    past_participle, self.view_range
                );
            } else {
                info!(
                    "cannot {} view range, it remains at {} (world range is {})",
                    infinitive, self.view_range, slice_bounds
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

    pub fn visible_chunks(&self) -> impl Iterator<Item = ChunkPosition> {
        let (min, max) = self.chunk_range;
        let xrange = min.0 - 1..=max.0 + 1; // +1 for little buffer
        let yrange = min.1 - 1..=max.1 + 1;

        xrange
            .cartesian_product(yrange)
            .map(|(x, y)| ChunkPosition(x, y))
    }

    pub fn world(&self) -> InnerWorldRef {
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

    pub fn set_chunk_bounds(&mut self, range: (ChunkPosition, ChunkPosition)) {
        self.chunk_range = range;
    }

    fn is_chunk_dirty(&self, chunk: &ChunkPosition) -> bool {
        !self.clean_chunks.contains(chunk)
    }

    pub fn mark_dirty(&mut self, chunk: ChunkPosition) {
        self.clean_chunks.remove(&chunk);
    }
}
