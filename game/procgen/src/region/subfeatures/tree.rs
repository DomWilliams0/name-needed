use crate::region::subfeature::{Rasterizer, Subfeature};
use unit::world::WorldPosition;

use crate::BlockType;
use common::Itertools;

#[derive(Debug)]
pub struct Tree {
    height: u8,
    canopy_size: (u8, u8),
}

impl Subfeature for Tree {
    fn rasterize(&mut self, root: WorldPosition, rasterizer: &mut Rasterizer) {
        // TODO actual tree shape
        // just a column of random block for now
        for y in 0..self.height as i32 {
            let mut trunk = root;
            trunk.2 += y; // ignore z overflow
            rasterizer.place_block(trunk, BlockType::TreeTrunk);
        }

        // "canopy"
        let (w, h) = self.canopy_size;
        let (w, h) = (w as i32, h as i32);
        for (x, y) in (-w..w).cartesian_product(-h..h) {
            let pos = root + (x, y, self.height as i32);
            rasterizer.place_block(pos, BlockType::Leaves);
        }
    }
}

impl Tree {
    pub fn new(height: u8, canopy_size: (u8, u8)) -> Self {
        // TODO tree configuration based on its planet location - branch count, leaf spread, etc
        Self {
            height,
            canopy_size,
        }
    }
}
