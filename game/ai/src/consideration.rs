use common::bumpalo::Bump;
use common::*;
use std::fmt::{Debug, Formatter, Result as FmtResult};

use crate::context::pretty_type_name;
use crate::intelligence::InputCache;
use crate::Context;

#[derive(Clone, Copy)]
pub enum ConsiderationParameter {
    /// Already normalized
    Nop,
    Range {
        min: f32,
        max: f32,
    },
}

pub trait Consideration<C: Context> {
    fn curve(&self) -> Curve;
    fn input(&self) -> C::Input;
    fn parameter(&self) -> ConsiderationParameter;
    fn consider(
        &self,
        blackboard: &mut C::Blackboard,
        target: Option<&C::DseTarget>,
        input_cache: &mut InputCache<C>,
    ) -> NormalizedFloat {
        let input = input_cache.get(self.input(), blackboard, target);
        self.consider_input(input)
    }

    fn name(&self) -> &'static str {
        let name = pretty_type_name(std::any::type_name::<Self>());
        name.strip_suffix("Consideration").unwrap_or(name)
    }

    #[cfg(feature = "logging")]
    fn log_metric(&self, _: &str, _: f32) {}

    fn consider_input(&self, input: f32) -> NormalizedFloat {
        self.parameter().apply(input)
    }
}

/// For emitting considerations in a DSE
pub struct Considerations<'a, C: Context> {
    // TODO dont bother running destructors
    vec: BumpVec<'a, &'a dyn Consideration<C>>,
    alloc: &'a Bump,
}

impl ConsiderationParameter {
    pub fn apply(self, value: f32) -> NormalizedFloat {
        match self {
            ConsiderationParameter::Nop => NormalizedFloat::new(value),
            ConsiderationParameter::Range { min, max } => {
                NormalizedFloat::clamped((value - min) / (max - min))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Curve {
    /// x
    Identity,

    /// ax + b
    ///
    /// (mx+c)
    Linear(f32, f32),

    /// (ax^2 + bx + c)
    Quadratic(f32, f32, f32),

    /// d(a^(bx+c)) + e
    Exponential(f32, f32, f32, f32, f32),

    /// a + (b * sqrt(c * x))
    SquareRoot(f32, f32, f32),
}

impl Curve {
    #[allow(clippy::many_single_char_names)]
    pub fn evaluate(&self, x: NormalizedFloat) -> NormalizedFloat {
        let x = x.value();
        NormalizedFloat::clamped(match self {
            Curve::Identity => x,
            Curve::Linear(m, c) => (m * x) + c,
            Curve::Quadratic(a, b, c) => (a * x.powi(2) + (b * x) + c),
            Curve::Exponential(a, b, c, d, e) => (a.powf((b * x) + c) * d) + e,
            Curve::SquareRoot(a, b, c) => (a + (b * (c * x).sqrt())),
        })
    }
}

impl<'a, C: Context> Debug for Considerations<'a, C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_list()
            .entries(self.vec.iter().map(|c| c.name()))
            .finish()
    }
}

impl<'a, C: Context> Considerations<'a, C> {
    pub fn new(alloc: &'a Bump) -> Self {
        Considerations {
            vec: BumpVec::new_in(alloc),
            alloc,
        }
    }

    pub fn add<T: Consideration<C> + 'a>(&mut self, c: T) {
        assert!(
            !std::mem::needs_drop::<T>(),
            "drop won't be run for consideration"
        );
        let c = self.alloc.alloc(c) as &dyn Consideration<C>;
        self.vec.push(c)
    }

    pub fn into_vec(self) -> BumpVec<'a, &'a dyn Consideration<C>> {
        self.vec
    }

    pub fn drain(&mut self) -> impl Iterator<Item = &'a dyn Consideration<C>> + '_ {
        self.vec.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use common::{ApproxEq, NormalizedFloat};

    use crate::Curve;

    fn assert_eq(curve: Curve, x: f32, y: f32) {
        assert!(curve
            .evaluate(NormalizedFloat::clamped(x))
            .value()
            .approx_eq(y, (f32::EPSILON, 2)));
    }

    #[test]
    fn curves() {
        assert_eq(Curve::Linear(1.0, 0.0), 0.551, 0.551);
        assert_eq(Curve::Linear(2.0, 0.5), 0.25, 1.0);

        assert_eq(Curve::Quadratic(5.0, 2.0, -0.2), 0.2, 0.4);

        let expo = Curve::Exponential(2.0, -4.0, 0.0, -1.0, 1.0);
        assert_eq(expo.clone(), 0.0, 0.0);
        assert_eq(expo, 0.5, 0.75);
    }
}
