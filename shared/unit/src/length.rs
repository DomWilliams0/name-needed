use common::{derive_more::*, *};

/// Rough measurement of length. 1 = 10cm
#[derive(Constructor, Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From)]
pub struct Length(u16);

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From)]
pub struct Length3(Length, Length, Length);

impl Length {
    pub fn metres(self) -> f32 {
        (self.0 as f32) / 10.0
    }
}

impl Length3 {
    pub fn new(x: u16, y: u16, z: u16) -> Self {
        Length3(x.into(), y.into(), z.into())
    }

    pub const fn x(&self) -> Length {
        self.0
    }

    pub const fn y(&self) -> Length {
        self.1
    }

    pub const fn z(&self) -> Length {
        self.2
    }

    /// Does `other` fit into `self`
    pub fn fits(&self, other: &Self) -> bool {
        // checks the diagonal of `other` (the item) is not longer than that of `self` (the container)
        let Self(x, y, z) = *self;
        let Self(a, b, c) = *other;

        a.0.pow(2) + b.0.pow(2) + c.0.pow(2) <= x.0.pow(2) + y.0.pow(2) + z.0.pow(2)
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

#[cfg(test)]
mod tests {
    use crate::length::Length3;

    #[test]
    fn fits() {
        let rucksack = Length3::new(10, 10, 20);

        let apple = Length3::new(1, 1, 1);
        let baguette = Length3::new(1, 1, 15);
        let spear = Length3::new(1, 1, 30);

        assert!(rucksack.fits(&apple));
        assert!(rucksack.fits(&baguette));
        assert!(!rucksack.fits(&spear));
    }
}
