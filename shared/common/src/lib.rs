pub use cgmath;
pub use cgmath::{Angle, Deg, InnerSpace, MetricSpace, Rad, Rotation3, VectorSpace, Zero};
#[cfg(feature = "binary")]
pub use env_logger;
pub use float_cmp::ApproxEq;
pub use itertools::*;
pub use log::*;
pub use num_traits;
pub use rand::prelude::*;
pub use struclog::{
    self, enter_span, event_error, event_info, event_trace, event_verbose, EntityEvent, Event, Span,
};

pub use derive_more;
pub use lazy_static::lazy_static;

pub type F = f32;
pub type Vector3 = cgmath::Vector3<F>;
pub type Vector2 = cgmath::Vector2<F>;
pub type Point3 = cgmath::Point3<F>;
pub type Point2 = cgmath::Point2<F>;
pub type Matrix4 = cgmath::Matrix4<F>;
pub type Quaternion = cgmath::Quaternion<F>;

pub mod input;
