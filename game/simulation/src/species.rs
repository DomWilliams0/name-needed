use std::fmt::Write;
use std::rc::Rc;

use common::*;

use crate::ecs::*;
use crate::{CachedStr, StringCache};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Species(CachedStr);

/// Affects behaviours that interact with other members of the same species e.g. herding
#[derive(Component, EcsComponent, Clone, Debug, Eq, PartialEq)]
#[storage(DenseVecStorage)]
#[name("species")]
pub struct SpeciesComponent(Species);

impl SpeciesComponent {
    pub fn species(&self) -> Species {
        self.0
    }
}

impl Display for Species {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = self.0.as_ref();
        let chars = {
            let mut chars = s.chars();

            let first = chars.next().into_iter().flat_map(|c| c.to_uppercase());
            first.chain(chars)
        };

        for c in chars {
            f.write_char(c)?;
        }

        Ok(())
    }
}

impl<V: Value> ComponentTemplate<V> for SpeciesComponent {
    fn construct(
        values: &mut Map<V>,
        string_cache: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let name = values.get_string("name")?.to_lowercase(); // normalise case
        Ok(Rc::new(Self(Species(string_cache.get(&name)))))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

register_component_template!("species", SpeciesComponent);
