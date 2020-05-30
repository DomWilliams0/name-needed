use std::fmt::{Display, Error, Formatter};
use std::ops::Add;

use common::*;

use crate::mesh::BaseVertex;
use crate::{mesh, InnerWorldRef, WorldRef};
use unit::world::{ChunkPosition, SliceIndex};

/// Number of slices to see concurrently
const VIEW_RANGE: i32 = 3;

#[derive(Clone)]
pub struct WorldViewer {
    world: WorldRef,
    view_range: SliceRange,
    chunk_range: (ChunkPosition, ChunkPosition),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SliceRange(SliceIndex, SliceIndex);

impl SliceRange {
    fn new(start: SliceIndex, size: i32) -> Self {
        assert!(size > 0); // TODO Result
        Self(start, SliceIndex(start.0 + size))
    }

    pub fn from_bounds<F: Into<SliceIndex>, T: Into<SliceIndex>>(from: F, to: T) -> Self {
        Self(from.into(), to.into())
    }

    pub fn all() -> Self {
        Self(SliceIndex::MIN, SliceIndex::MAX)
    }

    pub fn null() -> Self {
        Self(SliceIndex(0), SliceIndex(0))
    }

    pub fn contains<S: Into<SliceIndex>>(self, slice: S) -> bool {
        let SliceIndex(slice) = slice.into();
        let SliceIndex(lower) = self.0;
        let SliceIndex(upper) = self.1;
        slice >= lower && slice <= upper
    }

    pub fn bottom(self) -> SliceIndex {
        self.0
    }
    pub fn top(self) -> SliceIndex {
        self.1
    }

    pub fn as_range(self) -> impl Iterator<Item = SliceIndex> {
        let SliceIndex(from) = self.0;
        let SliceIndex(to) = self.1;
        (from..=to).map(SliceIndex)
    }
}

impl Display for SliceRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let SliceRange(SliceIndex(from), SliceIndex(to)) = self;
        write!(f, "[{} => {}]", from, to)
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
        let start = SliceIndex(0);
        Self {
            world,
            view_range: SliceRange::new(start, VIEW_RANGE),
            chunk_range: (ChunkPosition(-1, -1), ChunkPosition(1, 1)),
        }
    }

    pub fn regenerate_dirty_chunk_meshes<F: FnMut(ChunkPosition, Vec<V>), V: BaseVertex>(
        &self,
        mut f: F,
    ) {
        let range = self.view_range;
        let world = self.world.borrow();
        for dirty_chunk in self
            .visible_chunks()
            .filter_map(|pos| world.find_chunk_with_pos(pos))
            .filter(|c| c.dirty())
        {
            let mesh = mesh::make_simple_render_mesh(dirty_chunk, range);
            f(dirty_chunk.pos(), mesh);
        }
    }

    fn invalidate_visible_chunks(&self) {
        // TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly
        let world = self.world.borrow();
        for c in self.visible_chunks() {
            if let Some(c) = world.find_chunk_with_pos(c) {
                c.invalidate()
            }
        }
    }

    pub fn move_by(&mut self, delta: i32) {
        let new_range = self.view_range + delta;
        let new_max = match delta.signum() {
            0 => return, // nop
            1 => new_range.1,
            -1 => new_range.0,
            _ => unreachable!(),
        };

        // TODO cache?
        let bounds = self.world.borrow().slice_bounds();
        if bounds.contains(new_max) {
            // in range
            self.view_range = new_range;
            self.invalidate_visible_chunks();
            info!("moved view range to {}", self.view_range);
        } else {
            info!(
                "cannot move view range, it remains at {} (world range is {})",
                self.view_range, bounds
            );
        }
    }

    pub fn visible_chunks(&self) -> impl Iterator<Item = ChunkPosition> {
        let (min, max) = self.chunk_range;
        let xrange = min.0..=max.0;
        let yrange = min.1..=max.1;

        xrange
            .cartesian_product(yrange)
            .map(|(x, y)| ChunkPosition(x, y))
    }

    pub fn world(&self) -> InnerWorldRef {
        self.world.borrow()
    }

    /*
        pub fn move_up(&mut self) {
            if self.view_range.move_up(1) {
                info!("moved view range to {}", self.view_range);
                self.invalidate_visible_chunks();
            } else {
                info!("cannot move view range, it remains at {}", self.view_range);
            }
        }
        pub fn move_down(&mut self) {
            if self.view_range.move_down(1) {
                info!("moved view range to {}", self.view_range);
                self.invalidate_visible_chunks();
            } else {
                info!("cannot move view range, it remains at {}", self.view_range);
            }
        }
    */

    //    pub fn goto(&mut self, new_slice: SliceIndex) { unimplemented!() }

    pub fn range(&self) -> SliceRange {
        self.view_range
    }

    pub fn set_chunk_bounds(&mut self, range: (ChunkPosition, ChunkPosition)) {
        self.chunk_range = range;
    }
}
