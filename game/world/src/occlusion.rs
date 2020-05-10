use std::fmt::{Debug, Formatter};
use std::hint::unreachable_unchecked;
use std::ops::Add;

use num_enum::TryFromPrimitive;

use common::derive_more::{Deref, DerefMut};
use unit::dim::CHUNK_SIZE;
use unit::world::{BlockCoord, BlockPosition, ChunkPosition};

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

impl NeighbourOffset {
    pub const COUNT: usize = 8;

    pub fn offsets() -> impl Iterator<Item = (NeighbourOffset, (i16, i16))> {
        OFFSETS.iter().copied()
    }

    /// SENW
    pub fn aligned() -> impl Iterator<Item = (NeighbourOffset, (i16, i16))> {
        OFFSETS.iter().step_by(2).copied()
    }

    /// SE NE NW SW
    pub fn corners() -> impl Iterator<Item = (NeighbourOffset, (i16, i16))> {
        OFFSETS.iter().skip(1).step_by(2).copied()
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
        OFFSETS[self as usize].1
    }

    pub fn is_aligned(self) -> bool {
        match self {
            NeighbourOffset::South
            | NeighbourOffset::East
            | NeighbourOffset::North
            | NeighbourOffset::West => true,
            _ => false,
        }
    }

    pub fn is_vertical(self) -> bool {
        debug_assert!(self.is_aligned());
        match self {
            NeighbourOffset::North | NeighbourOffset::South => true,
            _ => false,
        }
    }

    pub fn between_aligned(from: ChunkPosition, to: ChunkPosition) -> Self {
        let ChunkPosition(dx, dy) = to - from;
        let (dx, dy) = (dx.signum(), dy.signum());

        match (dx, dy) {
            (0, 1) => NeighbourOffset::North,
            (0, -1) => NeighbourOffset::South,
            (1, 0) => NeighbourOffset::East,
            (-1, 0) => NeighbourOffset::West,
            _ => unreachable!(),
        }
    }

    pub fn extend_across_boundary(self, mut pos: BlockPosition) -> BlockPosition {
        debug_assert!(self.is_aligned());

        match self {
            NeighbourOffset::South => pos.1 = CHUNK_SIZE.as_block_coord() - 1,
            NeighbourOffset::North => pos.1 = 0,
            NeighbourOffset::East => pos.0 = 0,
            NeighbourOffset::West => pos.0 = CHUNK_SIZE.as_block_coord() - 1,
            _ => {
                // safety: asserted self is aligned
                unsafe { unreachable_unchecked() }
            }
        };

        pos
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(u8)]
pub enum VertexOcclusion {
    /// Darkest
    Full = 0,
    Mostly = 1,
    Mildly = 2,
    /// No occlusion
    NotAtAll = 3,
}

impl Default for VertexOcclusion {
    fn default() -> Self {
        VertexOcclusion::NotAtAll
    }
}

impl VertexOcclusion {
    fn combine(self, other: Self) -> Self {
        if let (VertexOcclusion::Mildly, VertexOcclusion::Mildly) = (self, other) {
            VertexOcclusion::Mostly
        } else {
            self.min(other)
        }
    }
}

impl From<VertexOcclusion> for f32 {
    fn from(v: VertexOcclusion) -> Self {
        match v {
            VertexOcclusion::Full => 0.6,
            VertexOcclusion::Mostly => 0.7,
            VertexOcclusion::Mildly => 0.8,
            VertexOcclusion::NotAtAll => 1.0,
        }
    }
}

#[derive(Deref, DerefMut, Default, Copy, Clone)]
pub struct NeighbourOpacity(pub [Opacity; NeighbourOffset::COUNT]);

impl Debug for NeighbourOpacity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let solids = self
            .0
            .iter()
            .enumerate()
            .filter(|(_, o)| o.solid())
            .map(|(i, _)| {
                // safety: limited to NeighbourOffset::COUNT
                unsafe { std::mem::transmute::<_, NeighbourOffset>(i as u8) }
            });
        f.debug_list().entries(solids).finish()
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Opacity {
    Transparent = 0, // false
    Solid = 1,       // true
}

impl Default for Opacity {
    fn default() -> Self {
        Opacity::Transparent
    }
}

impl Opacity {
    pub fn solid(self) -> bool {
        if let Opacity::Solid = self {
            true
        } else {
            false
        }
    }

    pub fn transparent(self) -> bool {
        !self.solid()
    }
}

/// "Is occluded"
impl From<Opacity> for bool {
    fn from(o: Opacity) -> Self {
        o.solid()
    }
}

impl Add<VertexOcclusion> for VertexOcclusion {
    type Output = u8;

    fn add(self, rhs: VertexOcclusion) -> Self::Output {
        self as u8 + rhs as u8
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct BlockOcclusion([VertexOcclusion; 4]);

impl BlockOcclusion {
    pub fn from_neighbour_opacities(neighbours: NeighbourOpacity) -> Self {
        let get_vertex = |corner_offset: NeighbourOffset| -> VertexOcclusion {
            let s1 = neighbours[corner_offset.next() as usize];
            let s2 = neighbours[corner_offset.prev() as usize];

            let int_value = if s1.into() && s2.into() {
                0
            } else {
                let corner = neighbours[corner_offset as usize];
                3 - (s1 as u8 + s2 as u8 + corner as u8)
            };

            // Safety: value is 0 - 3
            unsafe { std::mem::transmute(int_value) }
        };

        Self([
            get_vertex(NeighbourOffset::SouthWest), // vertices 0 and 5
            get_vertex(NeighbourOffset::SouthEast), // vertex 1
            get_vertex(NeighbourOffset::NorthEast), // vertices 2 and 3
            get_vertex(NeighbourOffset::NorthWest), // vertex 4
        ])
    }

    pub fn should_flip(self) -> bool {
        let v = &self.0;
        v[0] + v[2] < v[1] + v[3]
    }

    /// Index must be <4. 0 is bottom left corner, goes anti clockwise
    pub fn corner(self, index: usize) -> VertexOcclusion {
        debug_assert!(index < 4);
        self.0[index]
    }

    pub fn update_from_neighbour_opacities(&mut self, neighbours: NeighbourOpacity) {
        let new = Self::from_neighbour_opacities(neighbours);
        self.0
            .iter_mut()
            .zip(new.0.iter())
            .for_each(|(a, b)| *a = (*a).combine(*b));
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;

    use super::*;

    #[test]
    fn offset_between_aligned_chunks() {
        assert_matches!(
            NeighbourOffset::between_aligned(ChunkPosition(5, 5), ChunkPosition(5, 6)),
            NeighbourOffset::North
        );
        assert_matches!(
            NeighbourOffset::between_aligned(ChunkPosition(5, 5), ChunkPosition(5, 1)),
            NeighbourOffset::South
        );

        assert_matches!(
            NeighbourOffset::between_aligned(ChunkPosition(-2, 5), ChunkPosition(-3, 5)),
            NeighbourOffset::West
        );
        assert_matches!(
            NeighbourOffset::between_aligned(ChunkPosition(-2, 5), ChunkPosition(33, 5)),
            NeighbourOffset::East
        );
    }
}
