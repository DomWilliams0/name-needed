mod declare;
mod grid_impl;

pub use grid_impl::{CoordRange, CoordType, DynamicGrid, Grid, GridImpl};

#[cfg(feature = "8neighbours")]
pub const NEIGHBOURS_COUNT: usize = 8;

#[cfg(not(feature = "8neighbours"))]
pub const NEIGHBOURS_COUNT: usize = 4;
