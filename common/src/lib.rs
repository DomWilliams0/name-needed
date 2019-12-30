pub use cgmath;
pub use itertools::Itertools;
pub use lazy_static::lazy_static;
pub use log::*;
pub use num_traits;
pub use rand::prelude::*;

#[cfg(feature = "binary")]
pub use env_logger;

pub type F = f32;
pub type Vector3 = cgmath::Vector3<F>;
pub type Vector2 = cgmath::Vector2<F>;
pub type Point3 = cgmath::Point3<F>;
pub use cgmath::{Angle, Deg, InnerSpace, Matrix4, MetricSpace, Rad};
