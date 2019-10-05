use std::convert::TryFrom;
use std::ops::RangeInclusive;

use generator::{done, Generator, Gn};

use crate::coordinate::world::{ChunkPosition, SliceIndex, SliceIndexType, CHUNK_SIZE};
use crate::mesh::Vertex;
use crate::{mesh, WorldRef};
use std::iter::Map;

/// Number of slices to see concurrently
const VIEW_RANGE: i32 = 3;

pub struct WorldViewer {
    world: WorldRef,
    view_range: SliceRange,
}

#[derive(Copy, Clone, Debug)]
pub struct SliceRange(SliceIndex, SliceIndex);

impl IntoIterator for SliceRange {
    type Item = SliceIndex;
    type IntoIter = Map<RangeInclusive<SliceIndexType>, fn(SliceIndexType) -> SliceIndex>; // yuk

    fn into_iter(self) -> Self::IntoIter {
        let SliceIndex(from) = self.0;
        let SliceIndex(to) = self.1;
        (from..=to).map(SliceIndex)
    }
}

impl SliceRange {
    fn new(start: SliceIndex, size: i32) -> Self {
        assert!(size > 0); // TODO Result?
        Self(start, SliceIndex(start.0 + size))
    }

    pub fn null() -> Self {
        Self(SliceIndex::MIN, SliceIndex::MAX)
    }

    /// returns true if it moved, otherwise false
    fn move_up(&mut self, delta: u32) -> bool {
        // TODO check if slice exists and allow unlimited movement
        // TODO cap to prevent panics for now
        let delta = i32::try_from(delta).expect("don't move so much");
        let new_upper = self.1 + delta;
        if new_upper.0 <= (CHUNK_SIZE - 1) as i32 {
            self.0 += delta;
            self.1 = new_upper;
            true
        } else {
            false
        }
    }

    /// returns true if it moved, otherwise false
    fn move_down(&mut self, delta: u32) -> bool {
        // TODO check if slice exists and allow unlimited movement
        // TODO cap to prevent panics for now
        let delta = i32::try_from(delta).expect("don't move so much");
        let new_lower = self.0 - delta;
        if new_lower.0 >= 0 {
            self.0 = new_lower;
            self.1 -= delta;
            true
        } else {
            false
        }
    }

    pub fn contains<S: Into<SliceIndex>>(self, slice: S) -> bool {
        let SliceIndex(slice) = slice.into();
        let SliceIndex(lower) = self.0;
        let SliceIndex(upper) = self.1;
        slice >= lower && slice <= upper
    }
}

impl WorldViewer {
    pub fn from_world(world: WorldRef) -> Self {
        let start = SliceIndex(0);
        Self {
            world,
            view_range: SliceRange::new(start, VIEW_RANGE),
        }
    }

    pub fn regen_dirty_chunk_meshes(&mut self) -> Generator<(), (ChunkPosition, Vec<Vertex>)> {
        Gn::new_scoped(move |mut s| {
            let range = self.view_range;
            for dirty_chunk in self.world.borrow().visible_chunks().filter(|c| c.dirty()) {
                let mesh = mesh::make_mesh(dirty_chunk, range);
                s.yield_((dirty_chunk.pos(), mesh));
            }

            done!();
        })
    }

    fn invalidate_visible_chunks(&self) {
        // TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly
        for c in self.world.borrow_mut().visible_chunks() {
            c.invalidate();
        }
    }

    pub fn move_up(&mut self) {
        if self.view_range.move_up(1) {
            self.invalidate_visible_chunks();
        }
    }
    pub fn move_down(&mut self) {
        if self.view_range.move_down(1) {
            self.invalidate_visible_chunks();
        }
    }

    //    pub fn goto(&mut self, new_slice: SliceIndex) { unimplemented!() }

    pub fn range(&self) -> SliceRange {
        self.view_range
    }
}
