use crate::world::BLOCKS_PER_METRE;
use misc::{derive_more::*, *};
use std::fmt::{Display, Formatter};
use std::ops::{Div, DivAssign};

/// Meters in xyz
#[derive(PartialOrd, PartialEq, Debug, Copy, Clone)]
pub struct Length3([f32; 3]);
impl Length3 {
    /// All dimensions must be finite and positive
    pub fn with_meters(xyz: [f32; 3]) -> Self {
        assert!(
            xyz.into_iter().all(|f| f.is_finite() && f >= 0.0),
            "bad length3 {:?}",
            xyz
        );
        Self(xyz)
    }

    pub const fn x(self) -> f32 {
        self.0[0]
    }

    pub const fn y(self) -> f32 {
        self.0[1]
    }

    pub const fn z(self) -> f32 {
        self.0[2]
    }

    pub const fn xyz(self) -> [f32; 3] {
        self.0
    }

    /// Does `other` fit into `self`
    pub fn fits(self, other: Self) -> bool {
        // checks the diagonal of `other` (the item) is not longer than that of `self` (the container)
        let [x, y, z] = self.0;
        let [a, b, c] = other.0;

        a.powi(2) + b.powi(2) + c.powi(2) <= x.powi(2) + y.powi(2) + z.powi(2)
    }
}

impl Display for Length3 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let [x, y, z] = self.0;
        write!(f, "{}x{}x{}", x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use crate::space::length::Length3;

    #[test]
    fn fits() {
        let rucksack = Length3::with_meters([1.0, 1.0, 2.0]);

        let apple = Length3::with_meters([0.1, 0.1, 0.1]);
        let baguette = Length3::with_meters([0.1, 0.1, 1.5]);
        let spear = Length3::with_meters([0.1, 0.1, 3.0]);

        assert!(rucksack.fits(apple));
        assert!(rucksack.fits(baguette));
        assert!(!rucksack.fits(spear));
    }
}
