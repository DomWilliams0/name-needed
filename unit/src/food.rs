use std::ops::Mul;

use misc::derive_more::{Add, Display};
use misc::newtype::Accumulative;

use misc::*;

/// Base unit of nutrition for hunger and eating
#[derive(Copy, Clone, Debug, Add, Display, Ord, PartialOrd, PartialEq, Eq)]
pub struct Nutrition(u16);

/// Rate at which food is burned and hunger increases. Represents amount of [Nutrition] to lose
/// per tick
#[derive(Copy, Clone, Debug)]
pub struct Metabolism(f32);

impl Accumulative for Nutrition {
    fn from_f32(val: f32) -> Self {
        Self(val as u16)
    }

    fn saturating_sub(self, x: Self) -> Self {
        Self(self.0.saturating_sub(x.0))
    }
}

impl Mul<NormalizedFloat> for Nutrition {
    type Output = Self;

    fn mul(self, rhs: NormalizedFloat) -> Self::Output {
        Self((self.0 as f32 * rhs.value()) as u16)
    }
}

impl Nutrition {
    pub fn new(val: u16) -> Self {
        Self(val)
    }

    pub fn proportion_of(self, max: Self) -> NormalizedFloat {
        // TODO casting to floats leads to loss of precision when large
        let div = self.0 as f64 / max.0 as f64;
        NormalizedFloat::clamped(div as f32)
    }

    pub fn remaining(self, max: Self) -> Nutrition {
        debug_assert!(max.0 >= self.0, "max should be bigger than value");
        Nutrition(max.0.saturating_sub(self.0))
    }
}

impl Metabolism {
    /// Must be positive
    pub fn new(val: f32) -> Option<Self> {
        if val.is_finite() && val > 0.0 {
            Some(Self(val))
        } else {
            None
        }
    }

    pub fn value(&self) -> f32 {
        self.0
    }
}
