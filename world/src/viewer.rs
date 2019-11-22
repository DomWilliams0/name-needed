use std::fmt::{Display, Error, Formatter};
use std::ops::Add;

use generator::{done, Generator, Gn};
use log::info;

use crate::coordinate::world::{ChunkPosition, SliceIndex};
use crate::mesh::Vertex;
use crate::{mesh, WorldRef};

/// Number of slices to see concurrently
const VIEW_RANGE: i32 = 3;

pub struct WorldViewer {
    world: WorldRef,
    view_range: SliceRange,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SliceRange(SliceIndex, SliceIndex);

impl SliceRange {
    fn new(start: SliceIndex, size: i32) -> Self {
        assert!(size > 0); // TODO Result?
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
        let rhs = SliceIndex(rhs);
        SliceRange(self.0 + rhs, self.1 + rhs)
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
                let mesh = mesh::make_render_mesh(dirty_chunk, range);
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
}
