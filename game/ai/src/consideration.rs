use common::NormalizedFloat;

use crate::{Context, Input};

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
    fn consider(&self, blackboard: &mut C::Blackboard) -> NormalizedFloat {
        self.parameter().apply(self.input().get(blackboard))
    }

    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
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

#[derive(Clone)]
pub enum Curve {
    Identity,
    Linear(f32, f32),
    Quadratic(f32, f32, f32),
    // d(a^(bx+c)) + e
    Exponential(f32, f32, f32, f32, f32),
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
        })
    }
}

#[cfg(test)]
mod tests {
    use std::f32::EPSILON;

    use common::{ApproxEq, NormalizedFloat};

    use crate::Curve;

    fn assert_eq(curve: Curve, x: f32, y: f32) {
        assert!(curve
            .evaluate(NormalizedFloat::clamped(x))
            .value()
            .approx_eq(y, (EPSILON, 2)));
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
