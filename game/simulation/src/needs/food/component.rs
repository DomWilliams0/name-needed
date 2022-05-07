use std::rc::Rc;

use common::*;
use unit::food::{Metabolism, Nutrition};

use crate::ecs::*;
use crate::needs::food::hunger::Hunger;
use crate::needs::food::FoodInterest;
use crate::StringCache;

#[derive(Component, EcsComponent, Debug, Clone)]
#[storage(DenseVecStorage)]
#[name("hunger")]
#[interactive]
#[clone(disallow)]
pub struct HungerComponent {
    hunger: Hunger,
    metabolism: Metabolism,
    food_interest: FoodInterest,
}

#[derive(Copy, Clone, Debug)]
pub enum EatType {
    /// Food is in inventory while eating
    Held,

    /// Food is nearby while eating
    Grazing,
}

/// A food item is being eaten by the given eater
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("being-eaten")]
#[clone(disallow)]
pub struct BeingEatenComponent {
    pub eater: Entity,
    pub ty: EatType,
}

impl HungerComponent {
    /// Defaults to full satiety
    pub fn new(max: Nutrition, metabolism: Metabolism, food_interest: FoodInterest) -> Self {
        Self {
            hunger: Hunger::new(max),
            metabolism,
            food_interest,
        }
    }

    pub fn hunger(&self) -> &Hunger {
        &self.hunger
    }
    pub fn hunger_mut(&mut self) -> &mut Hunger {
        &mut self.hunger
    }

    pub fn metabolism(&self) -> Metabolism {
        self.metabolism
    }

    pub fn food_interest(&self) -> &FoodInterest {
        &self.food_interest
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
        let max = Nutrition::new(values.get_int("max")?);
        let metabolism = Metabolism::new(values.get_float("metabolism")?).ok_or_else(|| {
            ComponentBuildError::TemplateSpecific("invalid metabolism".to_string())
        })?;
        let interest = values.get_string("interests")?.parse().map_err(|e| {
            ComponentBuildError::TemplateSpecific(format!("failed to parse interests: {e}"))
        })?;
        Ok(Rc::new(Self::new(max, metabolism, interest)))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

impl InteractiveComponent for HungerComponent {
    fn as_debug(&self) -> Option<&dyn Debug> {
        #[repr(transparent)]
        struct Interactive(HungerComponent);

        impl Debug for Interactive {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                write!(f, "Interests: ")?;
                let mut first = true;
                for (interest, pref) in self.0.food_interest.iter_interests() {
                    if !first {
                        write!(f, ", ")?
                    } else {
                        first = false;
                    }
                    write!(f, "{:?}={:.2}", interest, pref.value())?;
                }

                write!(f, "\nHunger: {:.4}", self.0.hunger().satiety().value())
            }
        }

        Some(unsafe { &*(self as *const HungerComponent as *const Interactive) })
    }
}
register_component_template!("hunger", HungerComponent);
