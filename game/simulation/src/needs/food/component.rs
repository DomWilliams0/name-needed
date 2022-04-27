use std::rc::Rc;

use common::newtype::AccumulativeInt;
use common::*;

use crate::ecs::*;
use crate::needs::food::FoodInterest;
use crate::StringCache;

// TODO newtype for Fuel
pub type Fuel = u16;

// TODO generic needs component with hunger/thirst/toilet/social etc
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("hunger")]
#[clone(disallow)]
pub struct HungerComponent {
    pub(in crate::needs::food) current_fuel: AccumulativeInt<Fuel>,
    max_fuel: Fuel,
    pub food_interest: FoodInterest,
}

/// A food item is being eaten by the given eater
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("being-eaten")]
#[clone(disallow)]
pub struct BeingEatenComponent {
    pub eater: Entity,
}

impl HungerComponent {
    pub fn new(max: Fuel, food_interest: FoodInterest) -> Self {
        Self {
            current_fuel: AccumulativeInt::new(max),
            max_fuel: max,
            food_interest,
        }
    }

    pub fn hunger(&self) -> NormalizedFloat {
        NormalizedFloat::new(self.current_fuel.value() as f32 / self.max_fuel as f32)
    }

    /// (a, b) -> a/b fuel
    pub fn satiety(&self) -> (Fuel, Fuel) {
        (self.current_fuel.value(), self.max_fuel)
    }

    pub fn set_satiety(&mut self, proportion: NormalizedFloat) {
        let fuel = self.max_fuel as f64 * proportion.value() as f64;
        self.current_fuel = AccumulativeInt::new(fuel as Fuel)
    }
}

impl<V: Value> ComponentTemplate<V> for HungerComponent {
    fn construct(
        values: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let max = values.get_int("max")?;
        let interest = values.get_string("interests")?.parse().map_err(|e| {
            ComponentBuildError::TemplateSpecific(format!("failed to parse interests: {e}"))
        })?;
        Ok(Rc::new(Self::new(max, interest)))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

register_component_template!("hunger", HungerComponent);
