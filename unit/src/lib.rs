mod coordinate;

// TODO use some fancy type safe unit conversion if this ever becomes a problem
mod scaling {
    /// 2 blocks per 1m
    pub const BLOCK_DIAMETER: f32 = 0.5;

    /// 1 human comfortably fits in 1m (i.e. 2x2 blocks)
    pub const HUMAN: f32 = 0.7;
}

pub use scaling::{BLOCK_DIAMETER, HUMAN};

pub use coordinate::*;
