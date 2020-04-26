use std::ops::Add;

#[derive(Copy, Clone, Debug)]
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
    pub fn offsets() -> impl Iterator<Item = (NeighbourOffset, (i16, i16))> {
        OFFSETS.iter().copied()
    }

    fn next(self) -> Self {
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

    fn prev(self) -> Self {
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
}

#[derive(Copy, Clone, Debug)]
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

impl From<VertexOcclusion> for f32 {
    fn from(v: VertexOcclusion) -> Self {
        match v {
            VertexOcclusion::Full => 0.5,
            VertexOcclusion::Mostly => 0.6,
            VertexOcclusion::Mildly => 0.8,
            VertexOcclusion::NotAtAll => 1.0,
        }
    }
}

impl Add<VertexOcclusion> for VertexOcclusion {
    type Output = u8;

    fn add(self, rhs: VertexOcclusion) -> Self::Output {
        self as u8 + rhs as u8
    }
}

#[derive(Default, Copy, Clone, Debug)]
// TODO bitset
pub struct BlockOcclusion([VertexOcclusion; 4]);

impl BlockOcclusion {
    pub fn from_neighbour_opacities(neighbours: [bool; 8]) -> Self {
        let get_vertex = |corner_offset: NeighbourOffset| -> VertexOcclusion {
            let s1 = neighbours[corner_offset.next() as usize];
            let s2 = neighbours[corner_offset.prev() as usize];

            let int_value = if s1 && s2 {
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
}
