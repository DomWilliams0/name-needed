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
// #[interactive] // TODO
#[clone(disallow)]
pub struct HungerComponent {
    hunger: Hunger,
    metabolism: Metabolism,
    pub food_interest: FoodInterest, // TODO getter
}

/// A food item is being eaten by the given eater
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("being-eaten")]
#[clone(disallow)]
pub struct BeingEatenComponent {
    pub eater: Entity,
    /// True for eating a held item, false for grazing
    pub is_equipped: bool, // TODO enum
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

    // pub(in crate::needs::food) fn consume_fuel(&mut self, nutrition: f32) {
    //     debug_assert!(nutrition.is_sign_positive());
    //     self.current_fuel -= nutrition;
    //     // TODO can this underflow?
    // }
    pub fn hunger(&self) -> &Hunger {
        &self.hunger
    }
    pub fn hunger_mut(&mut self) -> &mut Hunger {
        &mut self.hunger
    }

    pub fn metabolism(&self) -> Metabolism {
        self.metabolism
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

                todo!()
                // write!(f, "\nHunger: {:.2}", self.0.hunger().value())
            }
        }

        Some(unsafe { &*(self as *const HungerComponent as *const Interactive) })
    }
}
register_component_template!("hunger", HungerComponent);
