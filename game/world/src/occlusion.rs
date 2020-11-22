use common::derive_more::{Deref, DerefMut};
use common::*;

use crate::block::BlockOpacity;
use crate::neighbour::NeighbourOffset;
use std::ops::Add;

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum VertexOcclusion {
    /// Darkest
    Full,
    Mostly,
    Mildly,
    /// No occlusion
    NotAtAll,
}

impl Default for VertexOcclusion {
    fn default() -> Self {
        VertexOcclusion::NotAtAll
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

/// TODO bitset of Opacities will be much smaller, 2 bits each
#[derive(Deref, DerefMut, Default, Copy, Clone)]
pub struct NeighbourOpacity([OcclusionOpacity; NeighbourOffset::COUNT]);

impl NeighbourOpacity {
    pub const fn default_const() -> Self {
        // TODO this is different to the actual Default!
        Self([OcclusionOpacity::Known(BlockOpacity::Transparent); NeighbourOffset::COUNT])
    }

    /// Reduce to `[0 = transparent/unknown, 1 = solid]`
    fn opacities(&self) -> [u8; NeighbourOffset::COUNT] {
        // TODO return a transmuted u16 when bitset is used, much cheaper to create and compare
        [
            self.0[0].as_u8(),
            self.0[1].as_u8(),
            self.0[2].as_u8(),
            self.0[3].as_u8(),
            self.0[4].as_u8(),
            self.0[5].as_u8(),
            self.0[6].as_u8(),
            self.0[7].as_u8(),
        ]
    }

    #[cfg(test)]
    pub fn all_solid() -> Self {
        Self([OcclusionOpacity::Known(BlockOpacity::Solid); NeighbourOffset::COUNT])
    }
}

impl Debug for NeighbourOpacity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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
pub enum OcclusionOpacity {
    /// Across a chunk boundary, treated as transparent
    Unknown,
    Known(BlockOpacity),
}

impl Default for OcclusionOpacity {
    fn default() -> Self {
        OcclusionOpacity::Unknown
    }
}

impl OcclusionOpacity {
    pub fn solid(self) -> bool {
        match self {
            OcclusionOpacity::Unknown => false,
            OcclusionOpacity::Known(opacity) => opacity.solid(),
        }
    }

    pub fn transparent(self) -> bool {
        !self.solid()
    }

    fn as_u8(self) -> u8 {
        if self.solid() {
            1
        } else {
            0
        }
    }

    fn update(self, new: Self) -> Self {
        match (self, new) {
            (OcclusionOpacity::Unknown, known) | (known, OcclusionOpacity::Unknown) => known,
            (_, new) => new,
        }
    }
}

/// "Is occluded"
impl From<OcclusionOpacity> for bool {
    fn from(o: OcclusionOpacity) -> Self {
        o.solid()
    }
}

impl From<BlockOpacity> for OcclusionOpacity {
    fn from(o: BlockOpacity) -> Self {
        Self::Known(o)
    }
}

impl Add<VertexOcclusion> for VertexOcclusion {
    type Output = u8;

    fn add(self, rhs: VertexOcclusion) -> Self::Output {
        self as u8 + rhs as u8
    }
}

/// If a quad should be flipped for nicer AO
pub(crate) enum OcclusionFlip {
    Flip,
    DontFlip,
}

#[derive(Copy, Clone, Debug)]
pub struct BlockOcclusion(NeighbourOpacity);

impl BlockOcclusion {
    pub fn from_neighbour_opacities(neighbours: NeighbourOpacity) -> Self {
        Self(neighbours)
    }

    pub(crate) fn resolve_vertices(&self) -> ([VertexOcclusion; 4], OcclusionFlip) {
        let get_vertex = |corner_offset: NeighbourOffset| -> VertexOcclusion {
            let s1 = self.0[corner_offset.next() as usize];
            let s2 = self.0[corner_offset.prev() as usize];

            let int_value = if s1.into() && s2.into() {
                0
            } else {
                let corner = self.0[corner_offset as usize];
                3 - (s1.as_u8() + s2.as_u8() + corner.as_u8())
            };

            // Safety: value is 0 - 3
            unsafe { std::mem::transmute(int_value) }
        };

        let vertices = [
            get_vertex(NeighbourOffset::SouthWest), // vertices 0 and 5
            get_vertex(NeighbourOffset::SouthEast), // vertex 1
            get_vertex(NeighbourOffset::NorthEast), // vertices 2 and 3
            get_vertex(NeighbourOffset::NorthWest), // vertex 4
        ];

        let flip = if vertices[0] + vertices[2] < vertices[1] + vertices[3] {
            OcclusionFlip::Flip
        } else {
            OcclusionFlip::DontFlip
        };
        (vertices, flip)
    }

    pub const fn default_const() -> Self {
        Self(NeighbourOpacity::default_const())
    }

    pub fn update_from_neighbour_opacities(&mut self, neighbours: NeighbourOpacity) {
        (self.0)
            .0
            .iter_mut()
            .zip(neighbours.0.iter())
            .for_each(|(a, b)| *a = (*a).update(*b));
    }

    #[cfg(test)]
    pub fn corner(&self, i: usize) -> VertexOcclusion {
        let (vertices, _) = self.resolve_vertices();
        vertices[i]
    }
}

impl Default for BlockOcclusion {
    fn default() -> Self {
        Self::default_const()
    }
}

impl PartialEq<NeighbourOpacity> for BlockOcclusion {
    fn eq(&self, other: &NeighbourOpacity) -> bool {
        let my_opacities = self.0.opacities();
        let ur_opacities = other.opacities();
        my_opacities == ur_opacities
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;

    use unit::world::ChunkLocation;

    use super::*;

    #[test]
    fn offset_between_aligned_chunks() {
        assert_matches!(
            NeighbourOffset::between_aligned(ChunkLocation(5, 5), ChunkLocation(5, 6)),
            NeighbourOffset::North
        );
        assert_matches!(
            NeighbourOffset::between_aligned(ChunkLocation(5, 5), ChunkLocation(5, 1)),
            NeighbourOffset::South
        );

        assert_matches!(
            NeighbourOffset::between_aligned(ChunkLocation(-2, 5), ChunkLocation(-3, 5)),
            NeighbourOffset::West
        );
        assert_matches!(
            NeighbourOffset::between_aligned(ChunkLocation(-2, 5), ChunkLocation(33, 5)),
            NeighbourOffset::East
        );
    }

    #[test]
    fn after_block_removed() {
        let neighbour_occluded = {
            let mut o = NeighbourOpacity::default();
            o.0[0] = OcclusionOpacity::Known(BlockOpacity::Solid);
            o
        };
        let neighbour_not_occluded = {
            let mut o = NeighbourOpacity::default();
            o.0[0] = OcclusionOpacity::Known(BlockOpacity::Transparent);
            o
        };

        let mut occlusion = BlockOcclusion::from_neighbour_opacities(neighbour_occluded);
        assert_eq!(occlusion.corner(0), VertexOcclusion::Mildly);

        occlusion.update_from_neighbour_opacities(neighbour_not_occluded);
        assert_eq!(occlusion.corner(0), VertexOcclusion::NotAtAll);
    }
}
