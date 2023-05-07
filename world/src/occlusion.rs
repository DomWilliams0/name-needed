use misc::derive_more::{Deref, DerefMut};
use std::cell::Cell;
use std::convert::identity;
use std::fmt::{Debug, Formatter};

use crate::block::{Block, BlockOpacity};
use crate::chunk::slice::{IndexableSlice, Slice, SliceMut};
use crate::neighbour::NeighbourOffset;
use crate::WorldContext;
use std::ops::{Add, Index};
use unit::world::SliceBlock;

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

/// Holds opacity of 8 surrounding neighbours
/// TODO bitset of Opacities will be much smaller, 2 bits each
#[derive(Deref, DerefMut, Default, Copy, Clone)]
pub struct NeighbourOpacity([OcclusionOpacity; NeighbourOffset::COUNT]);

impl NeighbourOpacity {
    pub const fn default_const() -> Self {
        Self([OcclusionOpacity::Unknown; NeighbourOffset::COUNT])
    }

    pub const fn unknown() -> Self {
        Self([OcclusionOpacity::Unknown; NeighbourOffset::COUNT])
    }

    pub fn is_all_transparent(&self) -> bool {
        self.0.iter().all(|o| !o.solid())
    }

    pub fn is_all_solid(&self) -> bool {
        self.0.iter().all(|o| o.solid())
    }

    pub fn is_all_unknown(&self) -> bool {
        self.0
            .iter()
            .all(|o| matches!(o, OcclusionOpacity::Unknown))
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

    pub fn all_solid() -> Self {
        Self([OcclusionOpacity::Known(BlockOpacity::Solid); NeighbourOffset::COUNT])
    }

    pub fn all_transparent() -> Self {
        Self([OcclusionOpacity::Known(BlockOpacity::Transparent); NeighbourOffset::COUNT])
    }

    /// Top face only
    pub fn with_slice_above<C: WorldContext>(
        this_block: SliceBlock,
        slice_above: Slice<C>,
    ) -> NeighbourOpacity {
        // collect blocked state of each neighbour on the top face
        let mut opacity = NeighbourOpacity::default();
        for (n, offset) in NeighbourOffset::offsets() {
            if let Some(neighbour_block) = this_block.try_add(offset) {
                opacity[n as usize] =
                    OcclusionOpacity::Known(slice_above[neighbour_block].opacity());
            }
        }

        opacity
    }

    /// Sideways faces only (not top).
    /// extended_block: protruded from src block in direction of face
    pub fn with_neighbouring_slices<C: WorldContext>(
        extended_block: SliceBlock,
        this_slice: impl IndexableSlice<C>,
        slice_below: Option<Slice<C>>,
        slice_above: Option<Slice<C>>,
        face: OcclusionFace,
    ) -> NeighbourOpacity {
        debug_assert!(!matches!(face, OcclusionFace::Top));

        if this_slice.index(extended_block).opacity().solid() {
            // face is not visible
            return NeighbourOpacity::all_solid();
        }

        // defaults to unknown
        let mut opacity = NeighbourOpacity::default();

        enum Relative {
            SliceBelow,
            ThisSlice,
            SliceAbove,
        }

        // (which slice, movement in RELATIVE axis)
        // order matches NeighbourOffset::OFFSETS
        const RELATIVES: [(Relative, i16); 8] = [
            (Relative::SliceBelow, 0),
            (Relative::SliceBelow, 1),
            (Relative::ThisSlice, 1),
            (Relative::SliceAbove, 1),
            (Relative::SliceAbove, 0),
            (Relative::SliceAbove, -1),
            (Relative::ThisSlice, -1),
            (Relative::SliceBelow, -1),
        ];

        let (pos_idx, mul) = match face {
            OcclusionFace::North => (0, -1),
            OcclusionFace::East => (1, 1),
            OcclusionFace::South => (0, 1),
            OcclusionFace::West => (1, -1),
            OcclusionFace::Top => unreachable!(),
        };

        for (i, (relative, offset)) in RELATIVES.iter().enumerate() {
            let mut pos = [0, 0];
            // safety: idx is 0 or 1
            unsafe {
                *pos.get_unchecked_mut(pos_idx) = *offset * mul;
            }

            if let Some(pos) = extended_block.try_add((pos[0], pos[1])) {
                // TODO ideally check the slice first before calculating offset but whatever

                let neighbour_opacity = match (relative, slice_below, slice_above) {
                    (Relative::SliceBelow, Some(slice), _) => slice[pos].opacity(),
                    (Relative::SliceAbove, _, Some(slice)) => slice[pos].opacity(),
                    (Relative::ThisSlice, _, _) => this_slice.index(pos).opacity(),
                    _ => continue,
                };

                opacity[i] = OcclusionOpacity::Known(neighbour_opacity);
            }
        }

        opacity
    }
}

impl Debug for NeighbourOpacity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO only for debugging
        let known = self.0.iter().enumerate().filter_map(|(i, o)| match o {
            OcclusionOpacity::Unknown => None,
            OcclusionOpacity::Known(o) => {
                // safety: limited to NeighbourOffset::COUNT
                let n = unsafe { std::mem::transmute::<_, NeighbourOffset>(i as u8) };
                Some((n, *o))
            }
        });

        f.debug_list().entries(known).finish()
    }
}

impl Debug for BlockOcclusion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let solid_only = !f.alternate();
        let entries = OcclusionFace::FACES
            .iter()
            .zip(self.neighbours.iter())
            .filter(|(f, n)| !(n.is_all_transparent() && solid_only));

        f.debug_map().entries(entries).finish()
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
        matches!(self, OcclusionOpacity::Known(BlockOpacity::Solid))
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

impl Add<VertexOcclusion> for VertexOcclusion {
    type Output = u8;

    fn add(self, rhs: VertexOcclusion) -> Self::Output {
        self as u8 + rhs as u8
    }
}

/// If a quad should be flipped for nicer AO
pub enum OcclusionFlip {
    Flip,
    DontFlip,
}

#[derive(Debug, Copy, Clone)]
#[repr(usize)]
pub enum OcclusionFace {
    Top,
    /// +Y
    North,
    /// +X
    East,
    /// -Y
    South,
    /// -X
    West,
    // dont ever see bottom
}

impl OcclusionFace {
    pub const COUNT: usize = 5;

    /// Not in same order as ordinal!!!
    pub const FACES: [OcclusionFace; Self::COUNT] = [
        OcclusionFace::South,
        OcclusionFace::West,
        OcclusionFace::East,
        OcclusionFace::Top,
        OcclusionFace::North,
    ];

    pub const SIDE_FACES: [OcclusionFace; Self::COUNT - 1] = [
        OcclusionFace::South,
        OcclusionFace::West,
        OcclusionFace::East,
        OcclusionFace::North,
    ];

    /// In same order as ordinal
    pub const ORDINALS: [OcclusionFace; Self::COUNT] = [
        OcclusionFace::Top,
        OcclusionFace::North,
        OcclusionFace::East,
        OcclusionFace::South,
        OcclusionFace::West,
    ];

    pub fn extend_sideways(self, pos: SliceBlock) -> Option<SliceBlock> {
        use OcclusionFace::*;
        match self {
            Top => None,
            North => pos.try_add((0, 1)),
            East => pos.try_add((1, 0)),
            South => pos.try_add((0, -1)),
            West => pos.try_add((-1, 0)),
        }
    }
}

#[derive(Copy, Clone)]
pub struct BlockOcclusion {
    /// Maps to [OcclusionFace::ORDINALS]
    neighbours: [NeighbourOpacity; OcclusionFace::COUNT],
}

#[derive(Default, Deref, Debug, Copy, Clone)]
pub struct BlockOcclusionUpdate([Option<NeighbourOpacity>; OcclusionFace::COUNT]);

impl BlockOcclusion {
    // TODO pub(crate)
    pub fn resolve_vertices(&self, face: OcclusionFace) -> ([VertexOcclusion; 4], OcclusionFlip) {
        let neighbours = self.neighbours[face as usize];
        let get_vertex = |corner_offset: NeighbourOffset| -> VertexOcclusion {
            let s1 = neighbours[corner_offset.next() as usize];
            let s2 = neighbours[corner_offset.prev() as usize];

            let int_value = if s1.solid() && s2.solid() {
                0
            } else {
                let corner = neighbours[corner_offset as usize];
                3 - (s1.as_u8() + s2.as_u8() + corner.as_u8())
            };

            // Safety: value is 0 - 3
            unsafe { std::mem::transmute(int_value) }
        };

        let vertices = [
            get_vertex(NeighbourOffset::SouthEast),
            get_vertex(NeighbourOffset::NorthEast),
            get_vertex(NeighbourOffset::NorthWest),
            get_vertex(NeighbourOffset::SouthWest),
        ];

        let flip = if vertices[0] + vertices[2] < vertices[1] + vertices[3] {
            OcclusionFlip::Flip
        } else {
            OcclusionFlip::DontFlip
        };
        (vertices, flip)
    }

    pub const fn all_transparent() -> Self {
        Self {
            neighbours: [NeighbourOpacity::unknown(); OcclusionFace::COUNT],
        }
    }

    pub const fn default_const() -> Self {
        Self {
            neighbours: [NeighbourOpacity::default_const(); OcclusionFace::COUNT],
        }
    }

    pub fn update_from_neighbour_opacities(&mut self, neighbours: &BlockOcclusionUpdate) {
        for (a, b) in self
            .neighbours
            .iter_mut()
            .zip(neighbours.iter())
            .filter_map(|(a, b)| b.map(|b| (a, b)))
        {
            for (a, b) in a.iter_mut().zip(b.iter()) {
                *a = a.update(*b);
            }
        }
    }

    pub fn set_face(&mut self, face: OcclusionFace, neighbours: NeighbourOpacity) {
        self.neighbours[face as usize] = neighbours;
    }

    pub fn get_face(&self, face: OcclusionFace) -> NeighbourOpacity {
        self.neighbours[face as usize]
    }

    pub fn visible_faces(&self) -> impl Iterator<Item = OcclusionFace> + '_ {
        self.neighbours
            .iter()
            .zip(OcclusionFace::ORDINALS.iter())
            .filter_map(|(n, &face)| if !n.is_all_solid() { Some(face) } else { None })
    }

    #[cfg(test)]
    pub fn top_corner(&self, i: usize) -> VertexOcclusion {
        let (vertices, _) = self.resolve_vertices(OcclusionFace::Top);
        vertices[i]
    }
}

impl BlockOcclusionUpdate {
    pub fn with_single_face(face: OcclusionFace, opacities: NeighbourOpacity) -> Self {
        let mut occ = [None; OcclusionFace::COUNT];
        occ[face as usize] = Some(opacities);
        Self(occ)
    }

    pub fn set_face(&mut self, face: OcclusionFace, opacity: NeighbourOpacity) {
        self.0[face as usize] = Some(opacity);
    }
}

impl Default for BlockOcclusion {
    fn default() -> Self {
        Self {
            neighbours: [NeighbourOpacity::unknown(); OcclusionFace::COUNT],
        }
    }
}

impl PartialEq<BlockOcclusionUpdate> for BlockOcclusion {
    /// Only compares Some faces against self's faces
    fn eq(&self, opacities: &BlockOcclusionUpdate) -> bool {
        for (i, ur_opacity) in opacities.iter().enumerate() {
            if let Some(ur_opacity) = ur_opacity {
                let my_opacity = self.neighbours[i];
                if my_opacity.opacities() != ur_opacity.opacities() {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::DummyWorldContext;
    use unit::world::ChunkLocation;

    #[test]
    fn offset_between_aligned_chunks() {
        assert!(matches!(
            NeighbourOffset::between_aligned(ChunkLocation(5, 5), ChunkLocation(5, 6)),
            NeighbourOffset::North
        ));
        assert!(matches!(
            NeighbourOffset::between_aligned(ChunkLocation(5, 5), ChunkLocation(5, 1)),
            NeighbourOffset::South
        ));

        assert!(matches!(
            NeighbourOffset::between_aligned(ChunkLocation(-2, 5), ChunkLocation(-3, 5)),
            NeighbourOffset::West
        ));
        assert!(matches!(
            NeighbourOffset::between_aligned(ChunkLocation(-2, 5), ChunkLocation(33, 5)),
            NeighbourOffset::East
        ));
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

        let mut occlusion = BlockOcclusion::default();
        occlusion.set_face(OcclusionFace::Top, neighbour_occluded);
        assert_eq!(occlusion.top_corner(0), VertexOcclusion::Mildly);

        occlusion.update_from_neighbour_opacities(&BlockOcclusionUpdate::with_single_face(
            OcclusionFace::Top,
            neighbour_not_occluded,
        ));
        assert_eq!(occlusion.top_corner(0), VertexOcclusion::NotAtAll);
    }
}
