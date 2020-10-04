use common::derive_more::*;
use common::OrderedFloat;
use std::ops::Mul;

/// Hunger fuel. The lower the more hungry
#[derive(Constructor, Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From)]
pub struct Hunger(u16);

/// Rate at which hunger is consumed
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From)]
pub struct Metabolism(OrderedFloat<f32>);

impl Metabolism {
    /// Should be positive
    pub const fn const_new(val: f32) -> Self {
        Metabolism(OrderedFloat(val))
    }

    pub fn new(val: f32) -> Self {
        debug_assert!(val.is_sign_positive(), "metabolism can't be negative");
        Metabolism(OrderedFloat(val))
    }

    const fn value(self) -> f32 {
        (self.0).0
    }
}

impl Mul<f32> for Metabolism {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.value()* rhs)
    }
}

impl Mul<Self> for Metabolism {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.value() * rhs.value())
    }
}
