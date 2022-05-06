use common::newtype::AccumulativeInt;
use common::NormalizedFloat;
use unit::food::{Metabolism, Nutrition};

#[derive(Debug, Clone)]
pub struct Hunger {
    max: Nutrition,
    current: AccumulativeInt<Nutrition>,
}

#[derive(Clone, Debug)]
pub struct FoodDescription {
    /// Total nutrition available from this item. Multiplied by item condition to get remaining
    /// nutrition
    pub total_nutrition: Nutrition,

    /// Max amount of this item that can be consumed (not digested) in 1 tick.
    /// Low: large meals, chewy fat. High: drinks, small foods
    pub consumption_rate: Nutrition,

    /// Proportion of the nutrition that is transferred to the eater
    pub efficiency: NormalizedFloat,
}

impl Hunger {
    /// Defaults to full satiety
    pub fn new(max: Nutrition) -> Self {
        Hunger {
            current: AccumulativeInt::new(max),
            max,
        }
    }

    /// (a, b) -> a/b nutrition
    pub fn satiety_raw(&self) -> (Nutrition, Nutrition) {
        (self.current.value(), self.max)
    }

    pub fn satiety(&self) -> NormalizedFloat {
        self.current.value().proportion_of(self.max)
    }

    pub fn set_satiety(&mut self, satiety: NormalizedFloat) {
        self.current = AccumulativeInt::new(self.max * satiety);
    }

    /// 1 tick's worth of eating. Returns proportion of food to consume
    pub fn eat(
        &mut self,
        food: &FoodDescription,
        food_condition: NormalizedFloat,
    ) -> NormalizedFloat {
        let food_remaining = food.total_nutrition * food_condition;
        let hunger_remaining = self.current.value().remaining(self.max);

        // calculate nutrition to consume from food
        let food_degradation = {
            // TODO vary eater speed
            let amount = food.consumption_rate;
            amount.min(food_remaining).min(hunger_remaining) // limited by amount of food remaining and hunger
        };

        // calculate amount of nutrition that the eater gains
        let hunger_increase = food_degradation * food.efficiency;

        // apply to hunger
        self.current.add(hunger_increase);
        debug_assert!(
            self.current.value() <= self.max,
            "hunger exceeds maximum nutrition"
        );

        // calculate proportion of food to degrade
        food_degradation.proportion_of(food.total_nutrition)
    }

    /// Panics on invalid exertion. Returns amount of fuel burned
    pub fn burn(&mut self, metabolism: Metabolism, exertion: f32) {
        let burned = metabolism.value() * exertion;
        if !(burned.is_finite() && burned.is_sign_positive()) {
            panic!("invalid exertion {exertion} for metabolism {metabolism:?}");
        }
        self.current -= burned;
    }
}
