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
        slice_this: Slice<'a, C>,
        slice_above: Option<Slice<'a, C>>,
        slice_below: Option<Slice<'a, C>>,
    },
    UpdateFromNeighbours {
        relative_slabs: RelativeSlabs<'a, C>,
    },
}

/// Holds opacity of 8 surrounding neighbours
/// TODO bitset of Opacities will be much smaller, 2 bits each
#[derive(Deref, DerefMut, Copy, Clone)]
pub struct NeighbourOpacity([BlockOpacity; NeighbourOffset::COUNT]);

impl NeighbourOpacity {
    pub const fn default_const() -> Self {
        Self::all_transparent()
    }

    pub fn is_all_transparent(&self) -> bool {
        self.0.iter().all(|o| o.transparent())
    }

    pub fn is_all_solid(&self) -> bool {
        self.0.iter().all(|o| o.solid())
    }

    pub const fn all_solid() -> Self {
        Self([BlockOpacity::Solid; NeighbourOffset::COUNT])
    }

    pub const fn all_transparent() -> Self {
        Self([BlockOpacity::Transparent; NeighbourOffset::COUNT])
    }

    /// Top face only
    pub async fn with_slice_above_other_slabs_possible<C: WorldContext>(
        orig_block: SlabPosition,
        ty: &mut OcclusionUpdateType<'_, C>,
        mut set_occlusion_idx: impl FnMut(usize, BlockOpacity),
    ) {
        // move source block in direction of face first
        let (slab_dz, rel_block_z) = {
            match orig_block.z().above() {
                Some(z) => (0, z),
                None => (1, LocalSliceIndex::bottom()),
            }
        };

        // check if block above is solid
        if match ty {
            OcclusionUpdateType::InitThisSlab {
                slice_above: Some(slice_above),
                ..
            } if slab_dz == 0 => IndexableSlice::index(slice_above, orig_block.to_slice_block())
                .opacity()
                .solid(),
            OcclusionUpdateType::UpdateFromNeighbours { relative_slabs } if slab_dz == 1 => {
                relative_slabs
                    .get_opacity(
                        [0, 0, 1],
                        orig_block.to_slice_block().to_slab_position(rel_block_z),
                    )
                    .await
                    .map(|op| op.solid())
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
                OcclusionUpdateType::InitThisSlab {
                    slice_above: Some(slice_above),
                    ..
                } if slab_offset == [0; 3] => {
                    let opacity = IndexableSlice::index(slice_above, slab_pos).opacity();
                    set_occlusion_idx(i, opacity);
                }
                OcclusionUpdateType::UpdateFromNeighbours { relative_slabs }
                    if slab_offset != [0; 3] =>
                {
                    let neighbour_opacity = relative_slabs
                        .get_opacity(slab_offset, slab_pos.to_slab_position(rel_block_z))
                        .await;

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
            OcclusionUpdateType::InitThisSlab {
                slice_this: this_slice,
                ..
            } if slab_dxy == [0; 2] => IndexableSlice::index(this_slice, rel_block)
                .opacity()
                .solid(),
            OcclusionUpdateType::UpdateFromNeighbours { relative_slabs } if slab_dxy != [0; 2] => {
                relative_slabs
                    .get_opacity(
                        [slab_dxy[0], slab_dxy[1], 0],
                        rel_block.to_slab_position(orig_block.z()),
                    )
                    .await
                    .map(|b| b.solid())
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
                0, // set just after
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
                    slice_this: this_slice,
                    slice_above,
                    slice_below,
                } if slab_offset == [0; 3] => {
                    let neighbour_opacity = match (relative, slice_below, slice_above) {
                        (Relative::SliceBelow, Some(slice), _) => slice[slab_pos].opacity(),
                        (Relative::SliceAbove, _, Some(slice)) => slice[slab_pos].opacity(),
                        (Relative::ThisSlice, _, _) => {
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
                        .get_opacity(slab_offset, slab_pos.to_slab_position(slab_z))
                        .await;

                    if let Some(op) = neighbour_opacity {
                        set_occlusion_idx(i, op);
                    }
                }
                _ => {}
            }
        }
    }
}

impl Default for NeighbourOpacity {
    fn default() -> Self {
        NeighbourOpacity::default_const()
    }
}

impl Debug for NeighbourOpacity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let faces = self.0.iter().enumerate().map(|(i, o)| {
            // safety: limited to NeighbourOffset::COUNT
            let n = unsafe { std::mem::transmute::<_, NeighbourOffset>(i as u8) };
            Some((n, *o))
        });

        f.debug_list().entries(faces).finish()
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
    Top = 0,
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
        OcclusionFace::North,
        OcclusionFace::East,
        OcclusionFace::South,
        OcclusionFace::West,
    ];

    /// In same order as ordinal
    pub const ORDINALS: [OcclusionFace; Self::COUNT] = [
        OcclusionFace::Top,
        OcclusionFace::North,
        OcclusionFace::East,
        OcclusionFace::South,
        OcclusionFace::West,
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

    pub fn offset_i32(self) -> (i32, i32, i32) {
        use OcclusionFace::*;
        match self {
            Top => (0, 0, 1),
            North => (0, 1, 0),
            East => (1, 0, 0),
            South => (0, -1, 0),
            West => (-1, 0, 0),
        }
    }

    pub fn offset_f32(self) -> (f32, f32, f32) {
        let (x, y, z) = self.offset_i32();
        (x as f32, y as f32, z as f32)
    }
}

#[derive(Copy, Clone)]
pub struct BlockOcclusion {
    /// Maps to [OcclusionFace::ORDINALS]
    neighbours: [NeighbourOpacity; OcclusionFace::COUNT],
}

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
                3 - (s1 as u8 + s2 as u8 + corner as u8)
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
            neighbours: [NeighbourOpacity::all_transparent(); OcclusionFace::COUNT],
        }
    }

    pub const fn default_const() -> Self {
        Self {
            neighbours: [NeighbourOpacity::default_const(); OcclusionFace::COUNT],
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
            .filter_map(|(n, &face)| (!n.is_all_solid()).then_some(face))
    }

    pub fn iter_faces(&self) -> impl Iterator<Item = (NeighbourOpacity, OcclusionFace)> + '_ {
        self.neighbours
            .iter()
            .zip(OcclusionFace::ORDINALS.iter())
            .map(|(n, f)| (*n, *f))
    }

    #[cfg(test)]
    pub fn top_corner(&self, i: usize) -> VertexOcclusion {
        let (vertices, _) = self.resolve_vertices(OcclusionFace::Top);
        vertices[i]
    }
}

impl Default for BlockOcclusion {
    fn default() -> Self {
        Self {
            neighbours: [NeighbourOpacity::all_transparent(); OcclusionFace::COUNT],
        }
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

    pub async fn get_opacity(
        &mut self,
        relative: [i8; 3],
        relative_block: SlabPosition,
    ) -> Option<BlockOpacity> {
        self.get(relative)
            .await
            .and_then(|slab| slab.get(SlabPositionAsCoord(relative_block)))
            .map(|b| b.opacity())
    }
}
