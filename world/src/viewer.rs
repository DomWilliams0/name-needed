use std::ops::RangeInclusive;

use generator::{done, Generator, Gn};

use crate::chunk::CHUNK_SIZE;
use crate::mesh;
use crate::mesh::Vertex;
use crate::{chunk, ChunkPosition, World, MAX_SLICE, MIN_SLICE};
use std::convert::TryFrom;

/// Number of slices to see concurrently
const VIEW_RANGE: i32 = 3;

pub struct WorldViewer<'a> {
    world: &'a mut World,

    view_range: SliceRange,
}

#[derive(Copy, Clone, Debug)]
pub struct SliceRange(chunk::SliceIndex, chunk::SliceIndex);

impl IntoIterator for SliceRange {
    type Item = chunk::SliceIndex;
    type IntoIter = RangeInclusive<chunk::SliceIndex>;

    fn into_iter(self) -> Self::IntoIter {
        self.0..=self.1
    }
}

impl SliceRange {
    fn new(start: chunk::SliceIndex, size: i32) -> Self {
        assert!(size > 0); // TODO Result?
        Self(start, start + size)
    }

    pub fn null() -> Self {
        Self(MIN_SLICE, MAX_SLICE)
    }

    /// returns true if it moved, otherwise false
    fn move_up(&mut self, delta: u32) -> bool {
        // TODO check if slice exists and allow unlimited movement
        // TODO cap to prevent panics for now
        let delta = i32::try_from(delta).expect("don't move so much");
        let new_upper = self.1 + delta;
        if new_upper <= (CHUNK_SIZE - 1) as i32 {
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
        if new_lower >= 0 {
            self.0 = new_lower;
            self.1 -= delta;
            true
        } else {
            false
        }
    }

    pub fn contains(self, slice: chunk::SliceIndex) -> bool {
        slice >= self.0 && slice <= self.1
    }
}

impl<'a> WorldViewer<'a> {
    pub fn from_world(world: &'a mut World) -> Self {
        let start = 0;
        Self {
            world,
            view_range: SliceRange::new(start, VIEW_RANGE),
        }
    }

    pub fn regen_dirty_chunk_meshes(&mut self) -> Generator<(), (ChunkPosition, Vec<Vertex>)> {
        Gn::new_scoped(move |mut s| {
            let range = self.view_range;
            for dirty_chunk in self.world.visible_chunks().filter(|c| c.dirty()) {
                let mesh = mesh::make_mesh(dirty_chunk, range);
                s.yield_((dirty_chunk.pos(), mesh));
            }

            done!();
        })
    }

    fn invalidate_visible_chunks(&mut self) {
        // TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly
        for c in self.world.visible_chunks() {
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
