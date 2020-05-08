#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EdgeCost {
    /// 1 high jump up
    JumpUp,

    /// 1 high jump down
    JumpDown,

    /// Flat walk
    Walk,
}

impl EdgeCost {
    pub fn weight(self) -> f32 {
        // TODO currently arbitrary, should depend on physical attributes
        match self {
            EdgeCost::JumpUp => 1.4,
            EdgeCost::JumpDown => 1.3,
            EdgeCost::Walk => 1.0,
        }
    }

    /// blocks assumed to be adjacent
    pub fn from_height_diff(z_diff: i32) -> Option<Self> {
        match z_diff {
            0 => Some(EdgeCost::Walk),
            1 => Some(EdgeCost::JumpUp),
            -1 => Some(EdgeCost::JumpDown),
            _ => None,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            EdgeCost::JumpUp => EdgeCost::JumpDown,
            EdgeCost::JumpDown => EdgeCost::JumpUp,
            EdgeCost::Walk => EdgeCost::Walk,
        }
    }

    pub fn z_offset(self) -> i32 {
        match self {
            EdgeCost::JumpUp => 1,
            EdgeCost::JumpDown => -1,
            EdgeCost::Walk => 0,
        }
    }
}
