use misc::{derive_more::*, *};
use std::fmt::{Display, Formatter};

/// Rough measurement of both mass and volume. 1 ~= 1 apple, i.e. ~100 grams
#[derive(
    Ord, PartialOrd, Eq, PartialEq, Debug, Copy, Clone, From, Add, AddAssign, Sub, SubAssign,
)]
pub struct Volume(u16);

impl Volume {
    pub fn new_direct(vol: u16) -> Self {
        Self(vol)
    }
    pub fn zero() -> Self {
        Self(0)
    }
    pub fn with_meters_cubed(m3: f32) -> Self {
        // 1 apple = 10x10x10 cm     = 0.001 m^3
        // 1 m^3   = 10x10x10 apples = 1000 apples
        Self((m3 * 1000.0).ceil() as u16)
    }
}

impl Display for Volume {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Volume {
    pub fn get(self) -> u16 {
        self.0
    }
}
