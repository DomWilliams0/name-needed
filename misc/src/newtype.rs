use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

use derive_more::Deref;
use num_traits::{clamp, clamp_max, AsPrimitive, NumCast, Saturating, Unsigned};

#[derive(Copy, Clone)]
pub struct Proportion<T> {
    value: T,
    max: T,
}

impl<T> Proportion<T>
where
    T: Unsigned + Copy + AsPrimitive<f32> + NumCast + PartialOrd<T> + Saturating + Debug,
{
    pub fn with_value(value: T, max: T) -> Self {
        let value = clamp_max(value, max);
        Self { value, max }
    }
    pub fn with_proportion(f: f32, max: T) -> Self {
        assert!(max > T::zero());
        Self {
            value: Self::value_from_proportion(f, max),
            max,
        }
    }

    pub fn set_proportion(&mut self, f: f32) {
        self.value = Self::value_from_proportion(f, self.max);
    }

    pub fn proportion(&self) -> f32 {
        debug_assert!(
            self.max != T::zero() || self.value != T::zero(),
            "proportion is 0/0??"
        );
        self.value.as_() / self.max.as_()
    }

    fn value_from_proportion(f: f32, max: T) -> T {
        T::from(f * max.as_()).unwrap()
    }

    pub fn value(&self) -> T {
        self.value
    }
}

impl Proportion<u8> {
    /// 0/0
    pub const fn default_empty() -> Self {
        Self { value: 0, max: 0 }
    }
}

impl<T: Saturating + Copy> SubAssign<T> for Proportion<T> {
    fn sub_assign(&mut self, rhs: T) {
        self.value = self.value.saturating_sub(rhs);
    }
}

// TODO support f64 too
#[derive(Copy, Clone, Default, PartialOrd, PartialEq, Debug, Deref)]
pub struct NormalizedFloat(f32);

impl NormalizedFloat {
    /// Panics if not 0..=1
    pub fn new(f: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&f),
            "{} out of range for normalized float",
            f
        );
        Self(f)
    }

    pub fn try_new(f: f32) -> Option<Self> {
            if (0.0..=1.0).contains(&f) {Some(Self(f))} else {None}
    }

    pub const fn zero() -> Self {
        Self(0.0)
    }
    pub const fn one() -> Self {
        Self(1.0)
    }

    pub fn clamped(f: f32) -> Self {
        Self(clamp(f, 0.0, 1.0))
    }

    pub const fn value(self) -> f32 {
        self.0
    }

    pub fn clamp_max(self, max: Self) -> Self {
        Self(self.0.max(max.0))
    }
}

impl From<NormalizedFloat> for f32 {
    fn from(f: NormalizedFloat) -> Self {
        f.0
    }
}

impl num_traits::FromPrimitive for NormalizedFloat {
    fn from_i64(_: i64) -> Option<Self> {
        None
    }

    fn from_u64(_: u64) -> Option<Self> {
        None
    }

    fn from_f64(n: f64) -> Option<Self> {
        if (0.0..=1.0).contains(&n) {
            Some(NormalizedFloat(n as f32))
        } else {
            None
        }
    }
}

impl SubAssign<f32> for NormalizedFloat {
    fn sub_assign(&mut self, rhs: f32) {
        *self = Self::clamped(self.0 - rhs)
    }
}

impl Sub<NormalizedFloat> for NormalizedFloat {
    type Output = Self;

    fn sub(self, rhs: NormalizedFloat) -> Self::Output {
        Self::clamped(self.0 - rhs.0)
    }
}

impl<T: Debug> Debug for Proportion<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Proportion({:?}/{:?})", self.value, self.max)
    }
}

impl<T: Display> Display for Proportion<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.value, self.max)
    }
}

impl Mul<Self> for NormalizedFloat {
    type Output = Self;

    fn mul(self, rhs: NormalizedFloat) -> Self::Output {
        dbg!(Self::new(self.0 * rhs.0))
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AccumulativeInt<T: Accumulative> {
    real_value: T,
    adjusted_value: T,
    acc: f32,
}

// T: Unsigned + Copy + FromPrimitive + AddAssign<T> + Saturating,
pub trait Accumulative: Copy + Add<Output = Self> {
    fn from_f32(val: f32) -> Self;
    fn saturating_sub(self, x: Self) -> Self;
}

impl<T: Accumulative> AccumulativeInt<T> {
    pub fn new(value: T) -> Self {
        Self {
            real_value: value,
            adjusted_value: value,
            acc: 0.0,
        }
    }

    pub fn value(&self) -> T {
        self.adjusted_value
    }

    pub fn add(&mut self, delta: T) {
        self.real_value = self.real_value + delta;
        self.update_value();
    }

    fn update_value(&mut self) {
        self.real_value = Self::add_float(self.real_value, self.acc.trunc());

        let delta = self.acc.fract();
        // let delta = if delta.is_sign_positive() {delta.ceil()} else {delta.floor()};
        self.adjusted_value = Self::add_float(self.real_value, delta.floor());

        self.acc = self.acc.fract();
    }

    #[inline]
    fn add_float(unsigned: T, delta: f32) -> T {
        let positive = delta.is_sign_positive();
        let delta = T::from_f32(delta.abs());
        if positive {
            unsigned + delta
        } else {
            unsigned.saturating_sub(delta)
        }
    }
}

impl<T: Accumulative> SubAssign<f32> for AccumulativeInt<T> {
    fn sub_assign(&mut self, rhs: f32) {
        self.acc -= rhs;
        self.update_value();
    }
}

impl<T: Accumulative> AddAssign<f32> for AccumulativeInt<T> {
    fn add_assign(&mut self, rhs: f32) {
        self.acc += rhs;
        self.update_value();
    }
}

macro_rules! impl_accumulative {
    ($ty:ty) => {
        impl Accumulative for $ty {
            fn from_f32(val: f32) -> Self {
                val as $ty
            }

            fn saturating_sub(self, x: Self) -> Self {
                self.saturating_sub(x)
            }
        }
    };
}

impl_accumulative!(u64);
impl_accumulative!(u16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulative_int() {
        const BIG: u64 = 18_446_744_073_709_551_610_u64;
        let mut int = AccumulativeInt::new(BIG);

        int -= 0.25;
        assert_eq!(int.value(), BIG - 1); // .75
        int -= 0.5;
        assert_eq!(int.value(), BIG - 1); // .25

        int -= 0.5;
        assert_eq!(int.value(), BIG - 2); // .75
        int += 0.3;
        assert_eq!(int.value(), BIG - 1); // .05
        int -= 0.15;
        assert_eq!(int.value(), BIG - 2); // .9

        int -= 4.5;
        assert_eq!(int.value(), BIG - 6); // .4

        int.add(0);
        assert_eq!(int.value(), BIG - 6); // unchanged

        int.add(2);
        assert_eq!(int.value(), BIG - 4);
    }

    #[test]
    #[should_panic]
    fn normalised_nan() {
        NormalizedFloat::new(f32::NAN);
    }

    #[test]
    fn decrement() {
        let mut val = AccumulativeInt::new(3000u16);

        val -= 0.6;
        assert_eq!(val.value(), 2999); // .4

        val -= 0.6;
        assert_eq!(val.value(), 2998); // .8

        val -= 0.9;
        assert_eq!(val.value(), 2997); // .9

        for _ in 0..50000 {
            val -= 0.0001;
        }
        assert_eq!(val.value(), 2992);
    }
}
