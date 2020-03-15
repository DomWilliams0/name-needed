use common::*;

use crate::chunk::slab::{SlabIndex, SLAB_SIZE};
use unit::dim::CHUNK_SIZE;
use unit::world::{BlockPosition, WorldPosition};

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

const CHUNK_BOUNDARY_SLICE_SIZE: usize = CHUNK_SIZE.as_usize() * SLAB_SIZE.as_usize();

// TODO ideally this would run at compile time with const fns, must must wait for rfc 57563
lazy_static! {
    static ref BOUNDARY_BLOCKS: [[BlockPosition; CHUNK_BOUNDARY_SLICE_SIZE]; 4] = {
        let mut blocks = [[(0,0,0).into(); CHUNK_BOUNDARY_SLICE_SIZE]; 4];

        let mut populate = |tup: (usize, &ChunkBoundary)| {
            // iterate in slabs for spacial locality
            let blocks = &mut blocks[tup.0];
            let mut cursor = 0;
            let write = |pos: BlockPosition| {
                blocks[cursor] = pos;
                cursor += 1;
            };

            match tup.1 {
                ChunkBoundary::Up => {
                    let y = CHUNK_SIZE.as_i32() - 1;
                    (0..SLAB_SIZE.as_i32())
                        .flat_map(move |z| {
                            (0..CHUNK_SIZE.as_i32()).map(move |x| (x, y, z as i32).into())
                        })
                        .for_each(write)
                }
                ChunkBoundary::Down => {
                    let y = 0;
                    (0..SLAB_SIZE.as_i32())
                        .flat_map(move |z| {
                            (0..CHUNK_SIZE.as_i32()).map(move |x| (x, y, z as i32).into())
                        })
                        .for_each(write)
                }
                ChunkBoundary::Left => {
                    let x = 0;
                    (0..SLAB_SIZE.as_i32())
                        .flat_map(move |z| {
                            (0..CHUNK_SIZE.as_i32()).map(move |y| (x, y, z as i32).into())
                        })
                        .for_each(write)
                }
                ChunkBoundary::Right => {
                    let x = CHUNK_SIZE.as_i32() - 1;
                    (0..SLAB_SIZE.as_i32())
                        .flat_map(move |z| {
                            (0..CHUNK_SIZE.as_i32()).map(move |y| (x, y, z as i32).into())
                        })
                        .for_each(write)
                }
            };

        };

        for tup in BOUNDARIES.iter().enumerate() {
            populate(tup);
        }

        blocks
    };
}

impl ChunkBoundary {
    pub fn boundaries() -> impl Iterator<Item = ChunkBoundary> {
        BOUNDARIES.iter().copied()
    }

    fn blocks(self) -> impl Iterator<Item = BlockPosition> {
        let idx = match self {
            ChunkBoundary::Up => 0usize,
            ChunkBoundary::Down => 1,
            ChunkBoundary::Left => 2,
            ChunkBoundary::Right => 3,
        };

        BOUNDARY_BLOCKS[idx].iter().copied()
    }

    pub fn blocks_in_slab(self, slab: SlabIndex) -> impl Iterator<Item = BlockPosition> {
        let z_offset = slab * SLAB_SIZE.as_i32();
        self.blocks()
            .map(move |pos| BlockPosition::from((pos.0, pos.1, pos.2 + z_offset)))
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
        assert_eq!(v.len(), CHUNK_SIZE.as_usize() * SLAB_SIZE.as_usize());
        assert!(v.iter().all(|b| {
            let y_is_constant = b.1 == CHUNK_SIZE.as_u16() - 1;
            let z_is_in_range_of_slab =
                (b.2).0 >= SLAB_SIZE.as_i32() && (b.2).0 < (SLAB_SIZE.as_i32() * 2);
            y_is_constant && z_is_in_range_of_slab
        }));
        assert_ne!(v[0], v[1]);
    }
}
