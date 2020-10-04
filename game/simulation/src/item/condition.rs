use common::*;
use std::ops::SubAssign;

#[derive(Debug, Clone)]
pub enum ItemConditionGrade {
    Broken,
    Terrible,
    Reasonable,
    Good,
    Superb,
    Perfect,
}

#[derive(Clone, Debug)]
pub struct ItemCondition {
    value: NormalizedFloat,

    /// Updated with value
    grade: ItemConditionGrade,
}

impl ItemCondition {
    pub fn perfect() -> Self {
        Self::new(NormalizedFloat::one())
    }

    pub fn new(proportion: NormalizedFloat) -> Self {
        Self {
            value: proportion,
            grade: ItemConditionGrade::from_proportion(proportion.value()),
        }
    }

    pub fn set(&mut self, proportion: NormalizedFloat) {
        *self = Self::new(proportion)
    }

    pub fn value(&self) -> NormalizedFloat {
        self.value
    }
}

impl ItemConditionGrade {
    fn from_proportion(proportion: f32) -> Self {
        match proportion {
            v if v <= 0.0 => ItemConditionGrade::Broken,
            v if v <= 0.2 => ItemConditionGrade::Terrible,
            v if v <= 0.55 => ItemConditionGrade::Reasonable,
            v if v <= 0.8 => ItemConditionGrade::Good,
            v if v <= 0.95 => ItemConditionGrade::Superb,
            _ => ItemConditionGrade::Perfect,
        }
    }
}

impl Display for ItemCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:?} ({})", self.grade, self.value.value())
    }
}

impl SubAssign<NormalizedFloat> for ItemCondition {
    fn sub_assign(&mut self, rhs: NormalizedFloat) {
        *self = Self::new(self.value - rhs)
    }
}
