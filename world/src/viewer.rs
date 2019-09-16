use crate::{chunk, slice, World};
use crate::block::Block;
use crate::chunk::{BLOCK_COUNT_CHUNK, CHUNK_SIZE};

const BLOCK_RENDER_SIZE: u32 = 16;
const VIEW_RANGE: i32 = 3;

pub struct WorldViewer<'a> {
    world: &'a mut World,

    current_lower: chunk::SliceIndex,
    current_upper: chunk::SliceIndex,

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
        let start = 0;
        Self {
            world,
            current_lower: start,
            current_upper: start + VIEW_RANGE,
            visible_meshes: Vec::with_capacity(BLOCK_COUNT_CHUNK), // max size to avoid more allocation
        }
    }

    /// Populates `self.visible_meshes` then returns it to be iterated
    pub fn visible_meshes(&mut self) -> &[SliceMesh] {
        self.visible_meshes.clear();

        for c in self.world.visible_chunks() {
            let slice_range = self.current_lower..=self.current_upper;

            for slice_index in slice_range {
                // TODO copy slice locally?
                let slice = c.slice(slice_index);

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
        }

        // TODO return iterator instead, the compiler has overpowered me
        &self.visible_meshes
    }


    pub fn move_up(&mut self) {
        // TODO check if slice exists and allow unlimited movement
        // TODO cap to prevent panics for now
        if self.current_upper < (CHUNK_SIZE - 1) as i32 {
            self.current_lower += 1;
            self.current_upper += 1;
        }
    }
    pub fn move_down(&mut self) {
        // TODO check if slice exists and allow unlimited movement
        // TODO cap to prevent panics for now
        if self.current_lower > 0 {
            self.current_lower -= 1;
            self.current_upper -= 1;
        }
    }

//    pub fn goto(&mut self, new_slice: SliceIndex) { unimplemented!() }
}

fn color_for_block(block: Block) -> (u8, u8, u8) {
    match block {
        Block::Air => (0, 0, 0),
        Block::Dirt => (192, 57, 43),
    }
}
