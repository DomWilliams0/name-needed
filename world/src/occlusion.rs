use misc::derive_more::{Deref, DerefMut};
use std::cell::Cell;
use std::convert::identity;
use std::fmt::{Debug, Formatter};
use std::future::Future;

use crate::block::{Block, BlockOpacity};
use crate::chunk::slice::{IndexableSlice, Slice, SliceMut};
use crate::neighbour::NeighbourOffset;
use crate::world::{get_or_wait_for_slab, ListeningLoadNotifier};
use crate::{Slab, WorldContext, WorldRef};
use grid::GridImpl;
use misc::{some_or_continue, trace, ArrayVec};
use std::ops::{Add, Index};
use unit::world::{
    LocalSliceIndex, SlabLocation, SlabPosition, SlabPositionAsCoord, SliceBlock, SliceIndex,
};

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

pub enum OcclusionUpdateType<'a, C: WorldContext> {
    InitThisSlab {
        this_slice: Slice<'a, C>,
        slice_above: Slice<'a, C>,
        slice_below: Option<Slice<'a, C>>,
    },
    UpdateFromNeighbours {
        relative_slabs: RelativeSlabs<'a, C>,
    },
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
    pub async fn with_slice_above_other_slabs_possible<C: WorldContext>(
        orig_block: SlabPosition,
        ty: &mut OcclusionUpdateType<'_, C>,
        mut set_occlusion_idx: impl FnMut(usize, BlockOpacity),
    ) {
        // TODO check if solid shortcut

        // move source block in direction of face first
        let (slab_dz, rel_block_z) = {
            match orig_block.z().above() {
                Some(z) => (0, z),
                None => (1, LocalSliceIndex::bottom()),
            }
        };

        // check if block above is solid
        if match ty {
            OcclusionUpdateType::InitThisSlab { slice_above, .. } if slab_dz == 0 => {
                IndexableSlice::index(slice_above, orig_block.to_slice_block())
                    .opacity()
                    .solid()
            }
            OcclusionUpdateType::UpdateFromNeighbours { relative_slabs } if slab_dz == 1 => {
                relative_slabs
                    .get([0, 0, 1])
                    .await
                    .and_then(|slab| {
                        slab.get(SlabPositionAsCoord(
                            orig_block.to_slice_block().to_slab_position(rel_block_z),
                        ))
                    })
                    .map(|b| b.opacity().solid())
                    .unwrap_or(false)
            }
            _ => false,
        } {
            for i in 0..NeighbourOffset::COUNT {
                set_occlusion_idx(i, BlockOpacity::Solid);
            }
            return;
        }

        for (i, (_, offset)) in NeighbourOffset::offsets().enumerate() {
            let (slab_offset_xy, slab_pos) =
                { orig_block.to_slice_block().try_add_intrusive(offset) };

            let slab_offset = [slab_offset_xy[0], slab_offset_xy[1], slab_dz];
            match ty {
                OcclusionUpdateType::InitThisSlab { slice_above, .. } if slab_offset == [0; 3] => {
                    let opacity = IndexableSlice::index(slice_above, slab_pos).opacity();
                    set_occlusion_idx(i, opacity);
                }
                OcclusionUpdateType::UpdateFromNeighbours { relative_slabs }
                    if slab_offset != [0; 3] =>
                {
                    let neighbour_opacity = relative_slabs
                        .get(slab_offset)
                        .await
                        .and_then(|slab| {
                            slab.get(SlabPositionAsCoord(slab_pos.to_slab_position(rel_block_z)))
                        })
                        .map(|b| b.opacity());

                    if let Some(op) = neighbour_opacity {
                        set_occlusion_idx(i, op);
                    }
                }
                _ => {}
            };
        }
    }

    /// Sideways faces only (not top).
    /// `block` is the solid block we are calculating all faces for. Can look into other slabs.
    /// Must not be top of slab slice?
    pub async fn with_neighbouring_slices_other_slabs_possible<C: WorldContext>(
        orig_block: SlabPosition,
        ty: &mut OcclusionUpdateType<'_, C>,
        face: OcclusionFace,
        mut set_occlusion_idx: impl FnMut(usize, BlockOpacity),
    ) {
        debug_assert!(!matches!(face, OcclusionFace::Top));

        #[derive(Debug)]
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

        // move source block in direction of face first
        let (slab_dxy, rel_block) = orig_block
            .to_slice_block()
            .try_add_intrusive(face.xy_delta());

        // check if block is solid, then we can skip everything else
        if match ty {
            OcclusionUpdateType::InitThisSlab { this_slice, .. } if slab_dxy == [0; 2] => {
                IndexableSlice::index(this_slice, rel_block)
                    .opacity()
                    .solid()
            }
            OcclusionUpdateType::UpdateFromNeighbours { relative_slabs } if slab_dxy != [0; 2] => {
                relative_slabs
                    .get([slab_dxy[0], slab_dxy[1], 0])
                    .await
                    .and_then(|slab| {
                        slab.get(SlabPositionAsCoord(
                            rel_block.to_slab_position(orig_block.z()),
                        ))
                    })
                    .map(|b| b.opacity().solid())
                    .unwrap_or(false)
            }
            _ => false,
        } {
            // set to all solid
            for i in 0..RELATIVES.len() {
                set_occlusion_idx(i, BlockOpacity::Solid);
            }

            return;
        }

        for (i, (relative, offset)) in RELATIVES.iter().enumerate() {
            let (slab_offset_xy, slab_pos) = {
                let mut pos = [0; 2];
                // safety: idx is 0 or 1
                unsafe {
                    *pos.get_unchecked_mut(pos_idx) = *offset * mul;
                }

                rel_block.try_add_intrusive((pos[0], pos[1]))
            };

            let mut slab_offset = [
                slab_offset_xy[0] + slab_dxy[0],
                slab_offset_xy[1] + slab_dxy[1],
                0,
            ];
            let slab_z = match relative {
                Relative::SliceAbove => match orig_block.z().above() {
                    Some(z) => z,
                    None => {
                        slab_offset[2] = 1;
                        LocalSliceIndex::bottom()
                    }
                },
                Relative::ThisSlice => orig_block.z(),
                Relative::SliceBelow => match orig_block.z().below() {
                    Some(z) => z,
                    None => {
                        slab_offset[2] = -1;
                        LocalSliceIndex::top()
                    }
                },
            };

            match ty {
                OcclusionUpdateType::InitThisSlab {
                    this_slice,
                    slice_above,
                    slice_below,
                } if slab_offset == [0; 3] => {
                    let neighbour_opacity = match (relative, slice_below) {
                        (Relative::SliceBelow, Some(slice)) => slice[slab_pos].opacity(),
                        (Relative::SliceAbove, _) => slice_above[slab_pos].opacity(),
                        (Relative::ThisSlice, _) => {
                            IndexableSlice::index(this_slice, slab_pos).opacity()
                        }
                        _ => continue,
                    };

                    set_occlusion_idx(i, neighbour_opacity);
                }
                OcclusionUpdateType::UpdateFromNeighbours { relative_slabs }
                    if slab_offset != [0; 3] =>
                {
                    let neighbour_opacity = relative_slabs
                        .get(slab_offset)
                        .await
                        .and_then(|slab| {
                            slab.get(SlabPositionAsCoord(slab_pos.to_slab_position(slab_z)))
                        })
                        .map(|b| b.opacity());

                    if let Some(op) = neighbour_opacity {
                        set_occlusion_idx(i, op);
                    }
                }
                _ => {}
            }
        }
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
        let entries = OcclusionFace::ORDINALS
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
pub enum OcclusionFace {
    /// +Y
    North,
    /// +X
    East,
    /// -Y
    South,
    /// -X
    West,
    Top,
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
        OcclusionFace::North,
        OcclusionFace::East,
        OcclusionFace::South,
        OcclusionFace::West,
    ];

    /// In same order as ordinal
    pub const ORDINALS: [OcclusionFace; Self::COUNT] = [
        OcclusionFace::North,
        OcclusionFace::East,
        OcclusionFace::South,
        OcclusionFace::West,
        OcclusionFace::Top,
    ];

    pub fn xy_delta(self) -> (i16, i16) {
        use OcclusionFace::*;
        match self {
            Top => (0, 0),
            North => (0, 1),
            East => (1, 0),
            South => (0, -1),
            West => (-1, 0),
        }
    }

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

    pub fn get_face_mut(&mut self, face: OcclusionFace) -> &mut NeighbourOpacity {
        &mut self.neighbours[face as usize]
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

pub struct RelativeSlabs<'a, C: WorldContext> {
    refs: ArrayVec<([i8; 3], Slab<C>), 18>,
    this_slab: SlabLocation,
    notifications: &'a mut ListeningLoadNotifier,
    world: &'a WorldRef<C>,
}

impl<'a, C: WorldContext> RelativeSlabs<'a, C> {
    pub fn new(
        this_slab: SlabLocation,
        notifications: &'a mut ListeningLoadNotifier,
        world: &'a WorldRef<C>,
    ) -> Self {
        Self {
            refs: Default::default(),
            this_slab,
            notifications,
            world,
        }
    }

    pub async fn get(&mut self, relative: [i8; 3]) -> Option<&Slab<C>> {
        assert_ne!(relative, [0; 3], "should not be in own slab");

        if let Some(found_idx) = self.refs.iter().position(|(rel, _)| (*rel == relative)) {
            return Some(&self.refs[found_idx].1);
        }

        let slab =
            get_or_wait_for_slab(self.notifications, self.world, self.this_slab + relative).await?;

        self.refs.push((relative, slab));

        Some(unsafe { &self.refs.get_unchecked(self.refs.len() - 1).1 })
    }
}
