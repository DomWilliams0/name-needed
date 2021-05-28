use common::{derive_more::*, *};
use std::ops::{Div, DivAssign};

/// Rough measurement of length. 1 = 10cm
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From)]
pub struct Length(u16);

/// 3D
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From)]
pub struct Length3(Length, Length, Length);

/// 2D
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From)]
pub struct Length2(Length, Length);

impl Length {
    /// How many in 1 metre
    const SCALE: f32 = 10.0;

    pub const fn new(len: u16) -> Self {
        Self(len)
    }
    pub fn metres(self) -> f32 {
        (self.0 as f32) / Self::SCALE
    }
}

impl Length3 {
    pub fn new(x: u16, y: u16, z: u16) -> Self {
        Length3(x.into(), y.into(), z.into())
    }

    pub const fn x(self) -> Length {
        self.0
    }

    pub const fn y(self) -> Length {
        self.1
    }

    pub const fn z(self) -> Length {
        self.2
    }

    /// Does `other` fit into `self`
    pub fn fits(self, other: Self) -> bool {
        // checks the diagonal of `other` (the item) is not longer than that of `self` (the container)
        let Self(x, y, z) = self;
        let Self(a, b, c) = other;

        a.0.pow(2) + b.0.pow(2) + c.0.pow(2) <= x.0.pow(2) + y.0.pow(2) + z.0.pow(2)
    }
}

impl Length2 {
    pub const fn x(self) -> Length {
        self.0
    }

    pub const fn y(self) -> Length {
        self.1
    }

    pub const fn xy(self) -> (Length, Length) {
        (self.0, self.1)
    }
}

impl Display for Length {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Display for Length3 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}x{}", self.0, self.1, self.2)
    }
}

impl Div<u16> for Length {
    type Output = Self;

    fn div(self, rhs: u16) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl DivAssign<u16> for Length {
    fn div_assign(&mut self, rhs: u16) {
        *self = Self(self.0 / rhs);
    }
}

impl From<Length3> for Length2 {
    fn from(len: Length3) -> Self {
        Self(len.0, len.1)
    }
}

#[cfg(test)]
mod tests {
    use crate::space::length::Length3;

    #[test]
    fn fits() {
        let rucksack = Length3::new(10, 10, 20);

        let apple = Length3::new(1, 1, 1);
        let baguette = Length3::new(1, 1, 15);
        let spear = Length3::new(1, 1, 30);

        assert!(rucksack.fits(apple));
        assert!(rucksack.fits(baguette));
        assert!(!rucksack.fits(spear));
    }
}
