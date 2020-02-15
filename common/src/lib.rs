pub use cgmath;
pub use float_cmp::ApproxEq;
pub use itertools::Itertools;
pub use lazy_static::lazy_static;
pub use log::*;
pub use num_traits;
pub use rand::prelude::*;
pub use struclog::{self, enter_span, event_error, event_info, event_trace, event_verbose,
                   EntityEvent, Event, Span};

#[cfg(feature = "binary")]
pub use env_logger;

pub type F = f32;
pub type Vector3 = cgmath::Vector3<F>;
pub type Vector2 = cgmath::Vector2<F>;
pub type Point3 = cgmath::Point3<F>;
pub type Point2 = cgmath::Point2<F>;
pub type Matrix4 = cgmath::Matrix4<F>;
pub use cgmath::{Angle, Deg, InnerSpace, MetricSpace, Rad, VectorSpace};
