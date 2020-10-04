use common::{derive_more::*, *};

/// Rough measurement of both mass and volume. 1 ~= 1 apple, i.e. ~100 grams
#[derive(
    Constructor,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Debug,
    Copy,
    Clone,
    From,
    Add,
    AddAssign,
    Sub,
    SubAssign,
)]
pub struct Volume(u16);

impl Display for Volume {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
