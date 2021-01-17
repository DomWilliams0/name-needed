use std::hint::unreachable_unchecked;
use std::marker::PhantomData;

use unit::world::CHUNK_SIZE;
use unit::world::{BlockCoord, BlockPosition, ChunkLocation};

pub struct Neighbours<B: NeighboursBehaviour, P: Into<[i32; 3]> + From<[i32; 3]>> {
    block: P,
    current: u8,
    _phantom: PhantomData<B>,
}
pub type SlabNeighbours<P> = Neighbours<Slab, P>;
pub type WorldNeighbours<P> = Neighbours<World, P>;

pub trait NeighboursBehaviour {
    fn range_check(x: i32, y: i32) -> bool;
}

pub struct Slab;
pub struct World;

#[derive(Copy, Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
#[repr(u8)]
pub enum NeighbourOffset {
    South,
    SouthEast,
    East,
    NorthEast,
    North,
    NorthWest,
    West,
    SouthWest,
}

impl NeighboursBehaviour for Slab {
    fn range_check(x: i32, y: i32) -> bool {
        x >= 0 && x < CHUNK_SIZE.as_i32() && y >= 0 && y < CHUNK_SIZE.as_i32()
    }
}

impl NeighboursBehaviour for World {
    fn range_check(_: i32, _: i32) -> bool {
        true
    }
}

impl<B: NeighboursBehaviour, P: Into<[i32; 3]> + From<[i32; 3]>> Neighbours<B, P> {
    const HORIZONTAL_OFFSETS: [(i32, i32); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];

    pub fn new(block: P) -> Self {
        Self {
            block,
            current: 0,
            _phantom: PhantomData,
        }
    }
}

impl<B: NeighboursBehaviour, P: Into<[i32; 3]> + From<[i32; 3]> + Clone> Iterator
    for Neighbours<B, P>
{
    type Item = P;

    fn next(&mut self) -> Option<Self::Item> {
        let [x, y, z] = self.block.clone().into();

        for (i, &(dx, dy)) in Self::HORIZONTAL_OFFSETS
            .iter()
            .enumerate()
            .skip(self.current as usize)
        {
            self.current = (i + 1) as u8;

            let n = {
                let (nx, ny) = (x + dx, y + dy);

                if !B::range_check(nx, ny) {
                    continue;
                }

                [nx, ny, z]
            };

            return Some(n.into());
        }

        None
    }
}

impl NeighbourOffset {
    pub const COUNT: usize = 8;

    const OFFSETS: [(NeighbourOffset, (i16, i16)); 8] = [
        (NeighbourOffset::South, (0, -1)),
        (NeighbourOffset::SouthEast, (1, -1)),
        (NeighbourOffset::East, (1, 0)),
        (NeighbourOffset::NorthEast, (1, 1)),
        (NeighbourOffset::North, (0, 1)),
        (NeighbourOffset::NorthWest, (-1, 1)),
        (NeighbourOffset::West, (-1, 0)),
        (NeighbourOffset::SouthWest, (-1, -1)),
    ];

    pub fn offsets() -> impl Iterator<Item = (NeighbourOffset, (i16, i16))> {
        Self::OFFSETS.iter().copied()
    }

    /// SENW
    pub fn aligned() -> impl Iterator<Item = (NeighbourOffset, (i16, i16))> {
        Self::OFFSETS.iter().step_by(2).copied()
    }

    /// SE NE NW SW
    pub fn corners() -> impl Iterator<Item = (NeighbourOffset, (i16, i16))> {
        Self::OFFSETS.iter().skip(1).step_by(2).copied()
    }

    //noinspection DuplicatedCode
    /// Clockwise
    pub(crate) fn next(self) -> Self {
        match self {
            NeighbourOffset::North => NeighbourOffset::NorthEast,
            NeighbourOffset::NorthEast => NeighbourOffset::East,
            NeighbourOffset::East => NeighbourOffset::SouthEast,
            NeighbourOffset::SouthEast => NeighbourOffset::South,
            NeighbourOffset::South => NeighbourOffset::SouthWest,
            NeighbourOffset::SouthWest => NeighbourOffset::West,
            NeighbourOffset::West => NeighbourOffset::NorthWest,
            NeighbourOffset::NorthWest => NeighbourOffset::North,
        }
    }

    //noinspection DuplicatedCode
    /// Anti-clockwise
    pub(crate) fn prev(self) -> Self {
        match self {
            NeighbourOffset::North => NeighbourOffset::NorthWest,
            NeighbourOffset::NorthEast => NeighbourOffset::North,
            NeighbourOffset::East => NeighbourOffset::NorthEast,
            NeighbourOffset::SouthEast => NeighbourOffset::East,
            NeighbourOffset::South => NeighbourOffset::SouthEast,
            NeighbourOffset::SouthWest => NeighbourOffset::South,
            NeighbourOffset::West => NeighbourOffset::SouthWest,
            NeighbourOffset::NorthWest => NeighbourOffset::West,
        }
    }

    //noinspection DuplicatedCode
    pub(crate) fn opposite(self) -> Self {
        match self {
            NeighbourOffset::North => NeighbourOffset::South,
            NeighbourOffset::NorthEast => NeighbourOffset::SouthWest,
            NeighbourOffset::East => NeighbourOffset::West,
            NeighbourOffset::SouthEast => NeighbourOffset::NorthWest,
            NeighbourOffset::South => NeighbourOffset::North,
            NeighbourOffset::SouthWest => NeighbourOffset::NorthEast,
            NeighbourOffset::West => NeighbourOffset::East,
            NeighbourOffset::NorthWest => NeighbourOffset::SouthEast,
        }
    }

    pub(crate) fn offset(self) -> (i16, i16) {
        Self::OFFSETS[self as usize].1
    }

    pub fn is_aligned(self) -> bool {
        matches!(
            self,
            NeighbourOffset::South
                | NeighbourOffset::East
                | NeighbourOffset::North
                | NeighbourOffset::West
        )
    }

    pub fn is_vertical(self) -> bool {
        debug_assert!(self.is_aligned());
        matches!(self, NeighbourOffset::North | NeighbourOffset::South)
    }

    pub fn between_aligned(from: ChunkLocation, to: ChunkLocation) -> Self {
        let ChunkLocation(dx, dy) = to - from;
        let (dx, dy) = (dx.signum(), dy.signum());

        match (dx, dy) {
            (0, 1) => NeighbourOffset::North,
            (0, -1) => NeighbourOffset::South,
            (1, 0) => NeighbourOffset::East,
            (-1, 0) => NeighbourOffset::West,
            _ => unreachable!(),
        }
    }

    pub fn extend_across_boundary(self, pos: BlockPosition) -> BlockPosition {
        debug_assert!(self.is_aligned());

        let (mut x, mut y, z) = pos.into();
        match self {
            NeighbourOffset::South => y = CHUNK_SIZE.as_block_coord() - 1,
            NeighbourOffset::North => y = 0,
            NeighbourOffset::East => x = 0,
            NeighbourOffset::West => x = CHUNK_SIZE.as_block_coord() - 1,
            _ => {
                // safety: asserted self is aligned
                unsafe { unreachable_unchecked() }
            }
        };

        BlockPosition::new(x, y, z)
    }

    pub fn position_on_boundary(self, other_coord: BlockCoord) -> (BlockCoord, BlockCoord) {
        debug_assert!(self.is_aligned());
        match self {
            NeighbourOffset::South => (other_coord, 0),
            NeighbourOffset::East => (CHUNK_SIZE.as_block_coord() - 1, other_coord),
            NeighbourOffset::North => (other_coord, CHUNK_SIZE.as_block_coord() - 1),
            NeighbourOffset::West => (0, other_coord),
            _ => {
                // safety: asserted self is aligned
                unsafe { unreachable_unchecked() }
            }
        }
    }
}
