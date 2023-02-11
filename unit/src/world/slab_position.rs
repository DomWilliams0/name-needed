use misc::derive_more::*;
use std::cmp::Ordering;

use crate::world::{
    BlockCoord, BlockPosition, LocalSliceIndex, SlabIndex, SlabLocation, SliceBlock, SliceIndex,
    WorldPosition, CHUNK_SIZE,
};
use misc::*;
use std::convert::TryFrom;

// TODO consider using same generic pattern as SliceIndex for all points and positions
//  e.g. single Position where x/y can be Global/Block, z is Global/Slab/None

/// A block in a slab
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Into, From)]
pub struct SlabPosition(BlockCoord, BlockCoord, LocalSliceIndex);

impl SlabPosition {
    /// None if x/y are out of range
    pub fn new(x: BlockCoord, y: BlockCoord, z: LocalSliceIndex) -> Option<Self> {
        if x < CHUNK_SIZE.as_block_coord() && y < CHUNK_SIZE.as_block_coord() {
            Some(Self(x, y, z))
        } else {
            None
        }
    }

    /// Panics if x/y are out of range
    pub fn new_unchecked(x: BlockCoord, y: BlockCoord, z: LocalSliceIndex) -> Self {
        Self::new(x, y, z).unwrap_or_else(|| panic!("coords out of range: {:?}", (x, y, z)))
    }

    pub fn to_world_position(self, slab: SlabLocation) -> WorldPosition {
        self.to_block_position(slab.slab)
            .to_world_position(slab.chunk)
    }

    pub fn to_slice_block(self) -> SliceBlock {
        SliceBlock::new_srsly_unchecked(self.0, self.1)
    }

    pub fn to_block_position(self, slab_index: SlabIndex) -> BlockPosition {
        BlockPosition::new_unchecked(self.0, self.1, self.2.to_global(slab_index))
    }

    pub const fn x(self) -> BlockCoord {
        self.0
    }
    pub const fn y(self) -> BlockCoord {
        self.1
    }
    pub const fn z(self) -> LocalSliceIndex {
        self.2
    }
}

// sort by z so slices are together, then y and x, so indices into slices are contiguous for contiguous blocks
impl PartialOrd for SlabPosition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.2
                .cmp(&other.2)
                .then(self.1.cmp(&other.1))
                .then(self.0.cmp(&other.0)),
        )
    }
}

impl Ord for SlabPosition {
    fn cmp(&self, other: &Self) -> Ordering {
        self.2
            .cmp(&other.2)
            .then(self.1.cmp(&other.1))
            .then(self.0.cmp(&other.0))
    }
}

impl TryFrom<[i32; 3]> for SlabPosition {
    type Error = ();

    fn try_from([x, y, z]: [i32; 3]) -> Result<Self, Self::Error> {
        match (
            BlockCoord::try_from(x),
            BlockCoord::try_from(y),
            LocalSliceIndex::new(z),
        ) {
            (Ok(x), Ok(y), Some(z)) => Self::new(x, y, z).ok_or(()),
            _ => Err(()),
        }
    }
}

impl From<SlabPosition> for [i32; 3] {
    fn from(p: SlabPosition) -> Self {
        let SlabPosition(x, y, z) = p;
        [i32::from(x), i32::from(y), z.slice() as i32]
    }
}

impl From<BlockPosition> for SlabPosition {
    fn from(p: BlockPosition) -> Self {
        Self::new_unchecked(p.x(), p.y(), p.z().to_local())
    }
}

impl From<WorldPosition> for SlabPosition {
    fn from(p: WorldPosition) -> Self {
        let p = BlockPosition::from(p);
        Self::new_unchecked(p.x(), p.y(), p.z().to_local())
    }
}

impl Display for SlabPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.0, self.1, self.2.slice())
    }
}
