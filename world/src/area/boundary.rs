use crate::chunk::slab::{SlabIndex, SLAB_SIZE};
use crate::coordinate::world::WorldPosition;
use crate::{BlockPosition, CHUNK_SIZE};

use std::mem::MaybeUninit;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ChunkBoundary {
    /// +y
    Up,

    /// -y
    Down,

    /// -x
    Left,

    /// +x
    Right,
}

const BOUNDARIES: [ChunkBoundary; 4] = [
    ChunkBoundary::Up,
    ChunkBoundary::Down,
    ChunkBoundary::Left,
    ChunkBoundary::Right,
];

// TODO ideally each boundary would have its own array, but i'll wait until const functions have
// stablized: rfc 57563
// and pls forgive me for the unsafe mess, this is only temporary after all ;¬)
const CHUNK_BOUNDARY_SLICE_SIZE: usize = CHUNK_SIZE.as_usize() * SLAB_SIZE;
static mut POSITIONS: [MaybeUninit<BlockPosition>; CHUNK_BOUNDARY_SLICE_SIZE] =
    [MaybeUninit::uninit(); CHUNK_BOUNDARY_SLICE_SIZE];

impl ChunkBoundary {
    pub fn boundaries() -> impl Iterator<Item = ChunkBoundary> {
        BOUNDARIES.iter().copied()
    }

    //noinspection DuplicatedCode
    pub fn blocks_in_slab(self, slab: SlabIndex) -> impl Iterator<Item = BlockPosition> {
        // iterate in slabs for spacial locality

        let mut cursor = 0;

        // offset z coords for slab index
        let z_coords = (0..SLAB_SIZE as i32).map(|z| z + (slab * SLAB_SIZE as i32));

        let write_awful = |pos| {
            unsafe { *POSITIONS[cursor].as_mut_ptr() = pos };
            cursor += 1;
        };

        match self {
            ChunkBoundary::Up => {
                let y = CHUNK_SIZE.as_i32() - 1;
                z_coords
                    .flat_map(move |z| {
                        (0..CHUNK_SIZE.as_i32()).map(move |x| (x, y, z as i32).into())
                    })
                    .for_each(write_awful)
            }
            ChunkBoundary::Down => {
                let y = 0;
                z_coords
                    .flat_map(move |z| {
                        (0..CHUNK_SIZE.as_i32()).map(move |x| (x, y, z as i32).into())
                    })
                    .for_each(write_awful)
            }
            ChunkBoundary::Left => {
                let x = 0;
                z_coords
                    .flat_map(move |z| {
                        (0..CHUNK_SIZE.as_i32()).map(move |y| (x, y, z as i32).into())
                    })
                    .for_each(write_awful)
            }
            ChunkBoundary::Right => {
                let x = CHUNK_SIZE.as_i32() - 1;
                z_coords
                    .flat_map(move |z| {
                        (0..CHUNK_SIZE.as_i32()).map(move |y| (x, y, z as i32).into())
                    })
                    .for_each(write_awful)
            }
        };

        // i know, i know...
        unsafe { POSITIONS.iter().map(|mem| mem.assume_init()) }
    }

    pub fn shift(self, pos: WorldPosition) -> WorldPosition {
        let WorldPosition(mut x, mut y, z) = pos;
        match self {
            ChunkBoundary::Up => y += 1,
            ChunkBoundary::Down => y -= 1,
            ChunkBoundary::Left => x -= 1,
            ChunkBoundary::Right => x += 1,
        };
        WorldPosition(x, y, z)
    }

    pub fn opposite(self) -> Self {
        match self {
            ChunkBoundary::Up => ChunkBoundary::Down,
            ChunkBoundary::Down => ChunkBoundary::Up,
            ChunkBoundary::Left => ChunkBoundary::Right,
            ChunkBoundary::Right => ChunkBoundary::Left,
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::area::ChunkBoundary;
    use crate::chunk::slab::SLAB_SIZE;

    use crate::CHUNK_SIZE;

    #[test]
    fn boundary() {
        let v: Vec<_> = ChunkBoundary::Up.blocks_in_slab(1).collect();
        assert_eq!(v.len(), CHUNK_SIZE.as_usize() * SLAB_SIZE);
        assert!(v.iter().all(|b| {
            let y_is_constant = (b.1).0 == CHUNK_SIZE.as_u16() - 1;
            let z_is_in_range_of_slab =
                (b.2).0 >= SLAB_SIZE as i32 && (b.2).0 < (SLAB_SIZE as i32 * 2);
            y_is_constant && z_is_in_range_of_slab
        }));
    }
}
