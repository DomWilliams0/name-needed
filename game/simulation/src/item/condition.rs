use common::{NormalizedFloat, Proportion};
use smallvec::alloc::fmt::Formatter;
use std::fmt::Display;

type Durability = u16;

#[derive(Debug, Clone)]
pub enum ItemConditionGrade {
    Broken,
    Terrible,
    Reasonable,
    Good,
    Superb,
    Perfect,
}

#[derive(Clone)]
pub struct ItemCondition {
    value: Proportion<Durability>,

    /// Updated with value
    grade: ItemConditionGrade,
}

impl ItemCondition {
    pub fn new_perfect(max: Durability) -> Self {
        Self::new(max, max)
    }

    pub fn with_proportion(durability: Proportion<Durability>) -> Self {
        Self {
            value: durability,
            grade: ItemConditionGrade::from_proportion(durability.proportion()),
        }
    }

    pub fn new(value: Durability, max: Durability) -> Self {
        let value = Proportion::with_value(value, max);
        Self::with_proportion(value)
    }

    /*
    pub fn decrement(&mut self) {
        self.value -= 1;
        self.grade = ItemConditionGrade::calculate(self.value, self.max)
    }
    */

    pub fn set(&mut self, proportion: NormalizedFloat) {
        self.value.set_proportion(proportion.value());
        self.grade = ItemConditionGrade::from_proportion(proportion.value());
    }

    pub fn value(&self) -> NormalizedFloat {
        NormalizedFloat::new(self.value.proportion())
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({:?})", self.value, self.grade)
    }
}
