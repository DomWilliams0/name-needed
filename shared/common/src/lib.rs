pub use cgmath;
pub use cgmath::{
    Angle, EuclideanSpace, InnerSpace, Matrix, MetricSpace, Rotation2, Rotation3, SquareMatrix,
    VectorSpace, Zero,
};
#[cfg(feature = "binary")]
pub use env_logger;
pub use float_cmp::ApproxEq;
pub use itertools::*;
pub use log::*;
pub use metrics::{self, declare_entity_metric, entity_metric};
pub use num_traits;
pub use ordered_float::OrderedFloat;
pub use rand::prelude::*;
pub use struclog::{self, *};

pub use derive_more;
pub use lazy_static::lazy_static;
pub use thiserror::Error;

pub type F = f32;
pub type Vector3 = cgmath::Vector3<F>;
pub type Vector2 = cgmath::Vector2<F>;
pub type Point3 = cgmath::Point3<F>;
pub type Point2 = cgmath::Point2<F>;
pub type Matrix4 = cgmath::Matrix4<F>;
pub type Quaternion = cgmath::Quaternion<F>;
pub type Basis2 = cgmath::Basis2<F>;
pub type Rad = cgmath::Rad<F>;
pub type Deg = cgmath::Deg<F>;

#[inline]
pub fn rad(f: F) -> Rad {
    cgmath::Rad(f)
}

#[inline]
pub fn deg(f: F) -> Deg {
    cgmath::Deg(f)
}

pub const AXIS_UP: Vector3 = Vector3::new(0.0, 0.0, 1.0);
pub const AXIS_FWD: Vector3 = Vector3::new(0.0, 1.0, 0.0);
pub const AXIS_FWD_2: Vector2 = Vector2::new(0.0, 1.0);

pub fn forward_angle(angle: Rad) -> Vector2 {
    use cgmath::{Basis2, Rotation};
    let rotation = Basis2::from_angle(-angle);
    rotation.rotate_vector(AXIS_FWD_2)
}

pub fn truncate(vec: Vector2, max: F) -> Vector2 {
    if vec.magnitude2() > (max * max) {
        vec.normalize_to(max)
    } else {
        vec
    }
}

pub use newtype::{NormalizedFloat, Proportion};

pub mod input;
pub mod newtype;
pub mod random;
