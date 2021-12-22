use common::Vector2;

use crate::World;
use unit::world::{WorldPoint, WorldPointRange, WorldPosition, WorldPositionRange, WorldRange};
use world::block::BlockOpacity;

#[derive(Clone, Debug)]
pub struct Bounds {
    range: WorldPointRange,
    block_range: WorldPositionRange,
    half_extents: Vector2,
}

pub trait BoundsCheck {
    fn all(&self, range: &WorldPositionRange) -> Option<BlockOpacity>;

    fn find_solids(&self, range: &WorldPositionRange, out: &mut Vec<WorldPosition>);
}

#[derive(Copy, Clone)]
pub enum BoundsOverlap {
    AllAir,
    AllSolid,
    None,
}

impl Bounds {
    pub fn from_radius(centre: WorldPoint, radius_x: f32, radius_y: f32) -> Self {
        // TODO vertical height too
        assert!(radius_x.is_sign_positive() && radius_y.is_sign_positive());

        let min = centre + (-radius_x, -radius_y, 0.0);
        let max = centre + (radius_x, radius_y, 0.0);

        Self {
            range: WorldRange::with_exclusive_range(min, max),
            block_range: WorldRange::with_exclusive_range(min.floor(), max.ceil()),
            half_extents: Vector2::new(radius_x, radius_y),
        }
    }

    fn move_by(&self, delta: (f32, f32, f32)) -> Self {
        let range = self.range.clone() + delta;
        let block_range = match range {
            WorldRange::Range(min, max) => {
                WorldRange::with_exclusive_range(min.floor(), max.ceil())
            }
            _ => unreachable!(), // only ranges used here
        };

        Self {
            range,
            block_range,
            half_extents: self.half_extents,
        }
    }

    pub fn into_range(self) -> WorldPositionRange {
        self.block_range
    }

    pub fn into_position(self) -> WorldPoint {
        let (min, _) = self.range.bounds();
        min + (self.half_extents.x, self.half_extents.y, 0.0)
    }

    pub fn check<B: BoundsCheck>(&self, check: &B) -> BoundsOverlap {
        Self::check_block_range(&self.block_range, check)
    }

    /// 1 thick range 1 block below
    pub fn check_ground<B: BoundsCheck>(&self, check: &B) -> BoundsOverlap {
        let ground_range = {
            let (min, mut max) = self.block_range.bounds();
            let min = min.below(); // ground below
            max.2 = min.2; // 1 thick

            WorldRange::with_inclusive_range(min, max)
        };

        Self::check_block_range(&ground_range, check)
    }

    fn check_block_range<B: BoundsCheck>(range: &WorldPositionRange, check: &B) -> BoundsOverlap {
        match check.all(range) {
            None => BoundsOverlap::None,
            Some(BlockOpacity::Solid) => BoundsOverlap::AllSolid,
            Some(BlockOpacity::Transparent) => BoundsOverlap::AllAir,
        }
    }

    pub fn find_solids<B: BoundsCheck>(&self, check: &B, out: &mut Vec<WorldPosition>) -> bool {
        check.find_solids(&self.block_range, out);
        !out.is_empty()
    }

    pub fn resolve_vertical_collision<B: BoundsCheck>(&self, check: &B) -> Self {
        // check below first
        let below = self.move_by((0.0, 0.0, -1.0));
        if let BoundsOverlap::AllAir = below.check(check) {
            below
        } else {
            // move upwards regardless
            self.move_by((0.0, 0.0, 1.0))
        }
    }
}

impl BoundsOverlap {
    pub fn is_all_air(self) -> bool {
        matches!(self, Self::AllAir)
    }
}

impl BoundsCheck for World {
    fn all(&self, range: &WorldPositionRange) -> Option<BlockOpacity> {
        let mut opacities = self
            .iterate_blocks(range)
            .map(|(block, _)| block.block_type().opacity());

        opacities.next().and_then(|opacity| {
            if opacities.all(|o| o == opacity) {
                // all equal to first
                Some(opacity)
            } else {
                // not all equal
                None
            }
        })
    }

    fn find_solids(&self, range: &WorldPositionRange, out: &mut Vec<WorldPosition>) {
        out.extend(self.iterate_blocks(range).filter_map(|(block, pos)| {
            if block.opacity().solid() {
                Some(pos)
            } else {
                None
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count() {
        // within a single block
        let bounds = Bounds::from_radius(WorldPoint::new_unchecked(0.5, 0.5, 0.0), 0.4, 0.2);
        assert_eq!(bounds.into_range().count(), 1);

        // just over the boundary in x axis
        let bounds = Bounds::from_radius(WorldPoint::new_unchecked(0.3, 0.5, 0.0), 0.4, 0.2);
        assert_eq!(bounds.into_range().count(), 2);

        // just over the boundary in y axis too
        let bounds = Bounds::from_radius(WorldPoint::new_unchecked(0.3, 0.3, 0.0), 0.4, 0.4);
        assert_eq!(bounds.into_range().count(), 4);

        // huge
        let bounds = Bounds::from_radius(WorldPoint::new_unchecked(10.0, 10.0, 5.0), 3.2, 3.2);
        assert_eq!(bounds.into_range().count(), 8 * 8);
    }
}
