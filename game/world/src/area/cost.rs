use float_cmp::ApproxEq;
use ordered_float::OrderedFloat;

use crate::block::BlockHeight;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EdgeCost {
    /// 1 high jump up
    JumpUp,

    /// 1 high jump down
    JumpDown,

    /// Flat walk
    Walk,

    /// A step up or down of the given height diff
    Step(OrderedFloat<f32>),
}

impl EdgeCost {
    pub fn weight(self) -> i32 {
        // TODO currently arbitrary, should depend on physical attributes
        match self {
            EdgeCost::JumpUp => 6,
            EdgeCost::JumpDown => 5,
            EdgeCost::Walk => 1,
            EdgeCost::Step(_) => 2, // TODO use height diff
        }
    }

    /// blocks assumed to be adjacent
    /// `z_diff` is the z difference between the blocks, e.g. +1 means `to` is 1 above `from`
    pub fn from_height_diff(
        from_height: BlockHeight,
        to_height: BlockHeight,
        z_diff: i32,
    ) -> Option<Self> {
        let diff = OrderedFloat((from_height.height() - to_height.height()) + z_diff as f32);
        let diff_abs = OrderedFloat(diff.abs());

        if diff.approx_eq(0.0, (0.0, 2)) {
            // 0 diff, simple walk
            Some(EdgeCost::Walk)
        } else if diff_abs > OrderedFloat(1.0) {
            // too big to jump or fall
            // TODO allow different jump sizes
            // TODO allow greater jump down distance
            None
        } else if diff_abs.approx_eq(1.0, (0.0, 2)) {
            // jump
            Some(if diff.is_sign_positive() {
                EdgeCost::JumpUp
            } else {
                EdgeCost::JumpDown
            })
        } else {
            // just a step of the difference
            Some(EdgeCost::Step(diff))
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            EdgeCost::JumpUp => EdgeCost::JumpDown,
            EdgeCost::JumpDown => EdgeCost::JumpUp,
            EdgeCost::Walk => EdgeCost::Walk,
            EdgeCost::Step(OrderedFloat(f)) => EdgeCost::Step(OrderedFloat(-f)),
        }
    }
}
