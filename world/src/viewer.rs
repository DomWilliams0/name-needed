use generator::{done, Generator, Gn};

use crate::chunk::CHUNK_SIZE;
use crate::mesh;
use crate::mesh::Vertex;
use crate::{chunk, ChunkPosition, World};

/// Number of slices to see concurrently
const VIEW_RANGE: i32 = 3;

pub struct WorldViewer<'a> {
    world: &'a mut World,

    current_lower: chunk::SliceIndex,
    current_upper: chunk::SliceIndex,
}

impl<'a> WorldViewer<'a> {
    pub fn from_world(world: &'a mut World) -> Self {
        let start = 0;
        Self {
            world,
            current_lower: start,
            current_upper: start + VIEW_RANGE,
        }
    }

    pub fn regen_dirty_chunk_meshes(&mut self) -> Generator<(), (ChunkPosition, Vec<Vertex>)> {
        Gn::new_scoped(move |mut s| {
            let range = (self.current_lower, self.current_upper);
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
        // TODO check if slice exists and allow unlimited movement
        // TODO cap to prevent panics for now
        if self.current_upper < (CHUNK_SIZE - 1) as i32 {
            self.current_lower += 1;
            self.current_upper += 1;
            self.invalidate_visible_chunks();
        }
    }
    pub fn move_down(&mut self) {
        // TODO check if slice exists and allow unlimited movement
        // TODO cap to prevent panics for now
        if self.current_lower > 0 {
            self.current_lower -= 1;
            self.current_upper -= 1;
            self.invalidate_visible_chunks();
        }
    }

    //    pub fn goto(&mut self, new_slice: SliceIndex) { unimplemented!() }
}
