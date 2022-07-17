use std::convert::{TryFrom, TryInto};

use std::marker::PhantomData;

use common::sized_iter::SizedIterator;
use common::Itertools;
use unit::world::CHUNK_SIZE;
use unit::world::{BlockCoord, BlockPosition, ChunkLocation};

pub struct Neighbours<B, P> {
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
#[cfg_attr(test, derive(Eq, PartialEq, Ord, PartialOrd))]
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

impl<B: NeighboursBehaviour, P> Neighbours<B, P> {
    const HORIZONTAL_OFFSETS: [(i32, i32); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];

    pub fn new(block: P) -> Self {
        Self {
            block,
            current: 0,
            _phantom: PhantomData,
        }
    }
}

impl<B: NeighboursBehaviour, P: Into<[i32; 3]> + TryFrom<[i32; 3]> + Clone> Iterator
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

            if let Ok(p) = n.try_into() {
                return Some(p);
            } else {
                continue;
            }
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
        assert!(self.is_aligned());
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

    pub fn extend_across_boundary_aligned(self, pos: BlockPosition) -> BlockPosition {
        use NeighbourOffset::*;
        assert!(self.is_aligned());

        let (mut x, mut y, z) = pos.into();
        match self {
            South => y = CHUNK_SIZE.as_block_coord() - 1,
            North => y = 0,
            East => x = 0,
            West => x = CHUNK_SIZE.as_block_coord() - 1,
            _ => unreachable!("should be aligned"),
        };

        BlockPosition::new_unchecked(x, y, z)
    }

    pub fn extend_across_any_boundary(
        self,
        source_block: BlockPosition,
        source_chunk: ChunkLocation,
    ) -> (BlockPosition, ChunkLocation) {
        use NeighbourOffset::*;
        const MAX: BlockCoord = CHUNK_SIZE.as_block_coord() - 1;

        let (mut bx, mut by, bz) = source_block.xyz();
        let (mut cx, mut cy) = source_chunk.xy();
        match self {
            South => {
                by = MAX;
                cy -= 1;
            }
            North => {
                by = 0;
                cy += 1;
            }
            East => {
                bx = 0;
                cx += 1;
            }
            West => {
                bx = MAX;
                cx -= 1;
            }
            SouthEast => {
                bx = 0;
                by = MAX;
                cx += 1;
                cy -= 1;
            }
            NorthEast => {
                bx = 0;
                by = 0;
                cx += 1;
                cy += 1;
            }
            SouthWest => {
                bx = MAX;
                by = MAX;
                cx -= 1;
                cy -= 1;
            }
            NorthWest => {
                bx = MAX;
                by = 0;
                cx -= 1;
                cy += 1;
            }
        };

        (
            BlockPosition::new_unchecked(bx, by, bz),
            ChunkLocation(cx, cy),
        )
    }

    pub fn position_on_boundary(self, other_coord: BlockCoord) -> (BlockCoord, BlockCoord) {
        debug_assert!(self.is_aligned());
        match self {
            NeighbourOffset::South => (other_coord, 0),
            NeighbourOffset::East => (CHUNK_SIZE.as_block_coord() - 1, other_coord),
            NeighbourOffset::North => (other_coord, CHUNK_SIZE.as_block_coord() - 1),
            NeighbourOffset::West => (0, other_coord),
            _ => unreachable!("should be aligned"),
        }
    }

    fn from_offset(offset: (i16, i16)) -> Option<NeighbourOffset> {
        use NeighbourOffset::*;
        Some(match offset {
            (0, -1) => South,
            (1, -1) => SouthEast,
            (1, 0) => East,
            (1, 1) => NorthEast,
            (0, 1) => North,
            (-1, 1) => NorthWest,
            (-1, 0) => West,
            (-1, -1) => SouthWest,
            _ => return None,
        })
    }

    pub fn accessible_neighbours(pos: BlockPosition) -> impl Iterator<Item = NeighbourOffset> {
        const MAX: BlockCoord = CHUNK_SIZE.as_block_coord() - 1;
        const MIN: BlockCoord = 0;
        let x_range = match pos.x() {
            MIN => -1..=0,
            MAX => 0..=1,
            _ => 0..=0,
        };
        let y_range = match pos.y() {
            MIN => -1..=0,
            MAX => 0..=1,
            _ => 0..=0,
        };

        let iter = x_range
            .cartesian_product(y_range)
            .filter_map(Self::from_offset);

        // maximum 3 results
        SizedIterator::new(iter, 3)
    }
}

#[cfg(test)]
mod tests {
    use common::Itertools;
    use unit::world::GlobalSliceIndex;

    use super::*;

    #[test]
    fn accessible_neighbours() {
        use NeighbourOffset::*;
        fn check(pos: (BlockCoord, BlockCoord), mut expected: Vec<NeighbourOffset>) {
            let block_pos =
                BlockPosition::new(pos.0, pos.1, GlobalSliceIndex::top()).expect("bad coords");

            expected.sort();
            let actual = NeighbourOffset::accessible_neighbours(block_pos)
                .sorted()
                .collect_vec();

            assert_eq!(expected, actual, "failed for {:?}", pos);
        }

        check((0, 0), vec![West, SouthWest, South]);
        check((2, 0), vec![South]);
        check((2, 2), vec![]);
        check((2, CHUNK_SIZE.as_block_coord() - 1), vec![North]);
        check(
            (
                CHUNK_SIZE.as_block_coord() - 1,
                CHUNK_SIZE.as_block_coord() - 1,
            ),
            vec![East, NorthEast, North],
        );
    }

    #[test]
    fn max_accessible_neighbours() {
        for (x, y) in
            (0..CHUNK_SIZE.as_block_coord()).cartesian_product(0..CHUNK_SIZE.as_block_coord())
        {
            let block_pos = BlockPosition::new_unchecked(x, y, GlobalSliceIndex::top());

            let n = NeighbourOffset::accessible_neighbours(block_pos).count();
            assert!(n <= 3, "too many, got {}", n);
        }
    }
}
