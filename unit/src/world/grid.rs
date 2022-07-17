use crate::world::{BlockCoord, LocalSliceIndex, RangePosition, SlabPosition, SliceBlock};
use misc::derive_more::{Deref, From};
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};

#[derive(Copy, Clone, Deref, From)]
pub struct SlabPositionAsCoord(pub SlabPosition);

#[derive(Copy, Clone, Deref, From)]
pub struct SliceBlockAsCoord(pub SliceBlock);

impl grid::CoordType for SlabPositionAsCoord {
    fn try_get(self) -> Option<[usize; 3]> {
        let (x, y, z) = self.0.xyz();
        // assume SlabPosition is correctly constructed and all coords are valid
        Some([x as usize, y as usize, z as usize])
    }

    fn from_coord([x, y, z]: [usize; 3]) -> Option<Self> {
        match (
            BlockCoord::try_from(x),
            BlockCoord::try_from(y),
            i32::try_from(z).ok().and_then(LocalSliceIndex::new),
        ) {
            (Ok(x), Ok(y), Some(z)) => Some(Self(SlabPosition::new_unchecked(x, y, z))),
            _ => None,
        }
    }
}

impl grid::CoordType for SliceBlockAsCoord {
    fn try_get(self) -> Option<[usize; 3]> {
        let (x, y) = self.0.xy();
        // assume SliceBlock is correctly constructed and all coords are valid
        Some([usize::from(x), usize::from(y), 0])
    }

    fn from_coord([x, y, _]: [usize; 3]) -> Option<Self> {
        match (BlockCoord::try_from(x), BlockCoord::try_from(y)) {
            (Ok(x), Ok(y)) => SliceBlock::new(x, y).map(Self),
            _ => None,
        }
    }
}

impl Debug for SlabPositionAsCoord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Debug for SliceBlockAsCoord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
