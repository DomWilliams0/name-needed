pub use dynamic::CoordRange;
pub use dynamic::DynamicGrid;
pub use grid_impl::{CoordType, Grid, GridImpl};

mod declare;
mod dynamic;
mod grid_impl;

#[cfg(feature = "8neighbours")]
pub const NEIGHBOURS_COUNT: usize = 8;

#[cfg(not(feature = "8neighbours"))]
pub const NEIGHBOURS_COUNT: usize = 4;
