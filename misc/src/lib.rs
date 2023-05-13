pub use arrayvec::*;
pub use bumpalo;
pub use tracy_client;

pub type BumpVec<'a, T> = bumpalo::collections::Vec<'a, T>;
pub type BumpString<'a> = bumpalo::collections::String<'a>;
pub type BumpBox<'a, T> = bumpalo::boxed::Box<'a, T>;

pub use cgmath::{
    self, Angle, EuclideanSpace, InnerSpace, Matrix, MetricSpace, Rotation2, Rotation3,
    SquareMatrix, VectorSpace, Zero,
};
pub use derivative::Derivative;
pub use derive_more;
pub use float_cmp::ApproxEq;
pub use itertools::*;
pub use num_traits;
pub use ordered_float::{NotNan, OrderedFloat};
pub use parking_lot;
pub use rand::{self, prelude::*};
pub use smallvec::{self, *};
pub use thiserror::{self, Error};

pub use lazy_static::lazy_static;
pub use logging::{
    self, prelude::*, slog_kv_debug, slog_kv_display, slog_value_debug, slog_value_display,
};
#[cfg(feature = "metrics")]
pub use metrics::{self, declare_entity_metric, entity_metric}; // nop macro declared below for disabled feature
pub use newtype::{NormalizedFloat, Proportion};

// misc imports that annoyingly get resolved to other pub exports of std/core
// https://github.com/intellij-rust/intellij-rust/issues/5654
pub use std::{
    error::Error,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    hash::Hash,
    iter::{empty, once},
    marker::PhantomData,
};

pub type BoxedResult<T> = Result<T, Box<dyn Error>>;

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

pub mod newtype;
pub mod sized_iter;

#[macro_export]
macro_rules! some_or_continue {
    ($opt:expr) => {
        match $opt {
            Some(v) => v,
            None => continue,
        }
    };
}
