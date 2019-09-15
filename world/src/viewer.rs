use std::cmp::{max, min};

use crate::{chunk, slice, World};
use crate::block::Block;
use crate::chunk::{BLOCK_COUNT_CHUNK, CHUNK_SIZE};

const BLOCK_RENDER_SIZE: u32 = 16;

pub struct WorldViewer<'a> {
    world: &'a mut World,
    current: chunk::SliceIndex,

    // reused between calls to visible_meshes
    visible_meshes: Vec<SliceMesh>,
}

#[derive(Copy, Clone, Debug)]
pub struct SliceMesh {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub color: (u8, u8, u8),
}

impl<'a> WorldViewer<'a> {
    pub fn from_world(world: &'a mut World) -> Self {
        Self {
            world,
            current: 0,
            visible_meshes: Vec::with_capacity(BLOCK_COUNT_CHUNK), // max size to avoid more allocation
        }
    }

    /// Populates `self.visible_meshes` then returns it to be iterated
    pub fn visible_meshes(&mut self) -> &[SliceMesh] {
        self.visible_meshes.clear();

        let current = self.current;
        for c in self.world.visible_chunks() {
            // TODO copy slice locally?
            let slice = c.slice(current);

            // get chunk offset
            let (cx, cy) = {
                let (x, y) = c.pos();
                (x * CHUNK_SIZE * BLOCK_RENDER_SIZE, y * CHUNK_SIZE * BLOCK_RENDER_SIZE)
            };

            // iterate non-air blocks
            for (i, b) in slice
                .iter()
                .enumerate()
                .filter(|(_i, b)| **b != Block::Air) {
                // unflatten slice index to 2d slice coords
                let (sx, sy) = slice::unflatten_index(i);

                // find rectangle render pos
                let x = (cx + (sx * BLOCK_RENDER_SIZE)) as i32;
                let y = (cy + (sy * BLOCK_RENDER_SIZE)) as i32;

                let mesh = SliceMesh {
                    x,
                    y,
                    width: BLOCK_RENDER_SIZE,
                    height: BLOCK_RENDER_SIZE,
                    color: color_for_block(*b),
                };

                self.visible_meshes.push(mesh);
            }
        }

        // TODO return iterator instead, the compiler has overpowered me
        &self.visible_meshes
    }


    pub fn move_up(&mut self) {
        // TODO check if slice exists and allow unlimited movement up
        self.current += 1;

        // TODO cap to prevent panics for now
        self.current = min(CHUNK_SIZE as i32 - 1, self.current);
    }
    pub fn move_down(&mut self) {
        // TODO check if slice exists and allow unlimited movement down
        self.current -= 1;

        // TODO cap to 0 to prevent panics for now
        self.current = max(0, self.current);
    }

//    pub fn goto(&mut self, new_slice: SliceIndex) { unimplemented!() }
}

fn color_for_block(block: Block) -> (u8, u8, u8) {
    match block {
        Block::Air => (0, 0, 0),
        Block::Dirt => (192, 57, 43),
    }
}
