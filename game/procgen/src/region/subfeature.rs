//! Rasterization of features to actual blocks via subfeatures

use crate::GeneratedBlock;

use dynstack::DynStack;
use unit::world::{ChunkLocation, SlabLocation, SlabPosition, WorldPosition};

/// Rasterizable object that places blocks within a slab, possibly leaking over the edge into other
/// slabs. In case of seepage, the subfeature is queued as a continuation for the neighbour slab.
///
/// Note the neighbour slab could already be loaded!!
pub trait Subfeature: Send {
    fn rasterize(&mut self, root: WorldPosition, rasterizer: &mut Rasterizer);
}

pub struct SlabContinuation {
    subfeatures: DynStack<dyn Subfeature>,
}

pub struct SlabNeighbour([i8; 3]);

/// Subfeatures call into a Rasterizer to place blocks, so the internals can change transparently
/// in the future. Currently it just plops all blocks into a vec and processes them afterwards, but
/// this will change to an async channel or something to avoid allocations/perf
pub struct Rasterizer {
    // TODO reuse borrowed vec allocations instead
    this_slab: Vec<(SlabPosition, GeneratedBlock)>,
    other_slabs: Vec<(SlabNeighbour, SlabPosition, GeneratedBlock)>,

    slab: SlabLocation,
}

impl Rasterizer {
    pub fn new(slab: SlabLocation) -> Self {
        Self {
            slab,
            this_slab: Vec::with_capacity(16),
            other_slabs: Vec::new(),
        }
    }

    pub fn place_block(&mut self, pos: WorldPosition, block: impl Into<GeneratedBlock>) {
        let block = block.into();
        let slab_pos = SlabPosition::from(pos);
        match resolve_slab(self.slab, pos) {
            None => self.this_slab.push((slab_pos, block)),
            Some(n) => self.other_slabs.push((n, slab_pos, block)),
        }
    }

    /// Call once
    pub fn internal_blocks(&mut self) -> impl Iterator<Item = (SlabPosition, GeneratedBlock)> {
        std::mem::take(&mut self.this_slab).into_iter()
    }

    /// Call once
    pub fn external_blocks(&mut self) -> Vec<(SlabNeighbour, SlabPosition, GeneratedBlock)> {
        std::mem::take(&mut self.other_slabs)
    }
}

/// None if within this slab, Some(diff) if within a neighbour. Direction is slab->neighbour
///
/// TODO handle case where block is multiple slabs over from root slab
fn resolve_slab(slab: SlabLocation, block: WorldPosition) -> Option<SlabNeighbour> {
    // (chunk x, chunk y, slab index)
    let [bx, by, bz]: [i32; 3] = {
        let z = block.slice().slab_index().as_i32();
        let (x, y) = ChunkLocation::from(block).xy();
        [x, y, z]
    };

    let [sx, sy, sz]: [i32; 3] = [slab.chunk.x(), slab.chunk.y(), slab.slab.as_i32()];

    // diff in this slab->block slab direction
    let diff = [bx - sx, by - sy, bz - sz];
    debug_assert!(
        diff.iter().all(|d| d.abs() <= 1),
        "slab is not adjacent (slab={:?}, block={:?}, diff={:?})",
        slab,
        block,
        diff
    );

    match diff {
        [0, 0, 0] => None,
        [dx, dy, dz] => Some(SlabNeighbour([dx as i8, dy as i8, dz as i8])),
    }
}
