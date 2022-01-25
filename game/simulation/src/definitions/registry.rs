use crate::definitions::builder::DefinitionBuilder;
use crate::definitions::{Definition, DefinitionErrorKind};

use crate::string::{CachedStr, CachedStringHasher, StringCache};
use crate::ComponentWorld;
use common::*;
use std::collections::HashMap;

pub struct DefinitionRegistry(HashMap<CachedStr, Definition, CachedStringHasher>);

pub struct DefinitionRegistryBuilder(HashMap<CachedStr, Definition, CachedStringHasher>);

impl DefinitionRegistryBuilder {
    pub fn new() -> Self {
        Self(HashMap::with_capacity_and_hasher(
            512,
            CachedStringHasher::default(),
        ))
    }

    pub fn register(
        &mut self,
        uid: CachedStr,
        definition: Definition,
    ) -> Result<(), (Definition, DefinitionErrorKind)> {
        #[allow(clippy::map_entry)]
        if self.0.contains_key(&uid) {
            Err((
                definition,
                DefinitionErrorKind::AlreadyRegistered(uid.as_ref().to_owned()),
            ))
        } else {
            self.0.insert(uid, definition);
            Ok(())
        }
    }

    pub fn build(self) -> DefinitionRegistry {
        info!(
            "creating definition registry with {count} entries",
            count = self.0.len()
        );
        DefinitionRegistry(self.0)
    }
}

impl DefinitionRegistry {
    pub fn instantiate<'s, 'w: 's, W: ComponentWorld>(
        &'s self,
        uid: &str,
        world: &'w W,
    ) -> Result<DefinitionBuilder<'s, W>, DefinitionErrorKind> {
        let uid = world.resource::<StringCache>().get(uid);
        match self.0.get(&uid) {
            Some(def) => Ok(DefinitionBuilder::new_with_cached(def, world, uid)),
            None => Err(DefinitionErrorKind::NoSuchDefinition(
                uid.as_ref().to_owned(),
            )),
        }
    }

    pub fn lookup_definition(&self, uid: CachedStr) -> Option<&Definition> {
        self.0.get(&uid)
    }

    pub fn iter_templates<T: 'static>(
        &self,
        component: &'static str,
    ) -> impl Iterator<Item = CachedStr> + '_ {
        self.0.iter().filter_map(move |(name, def)| {
            def.find_component(component).and_then(|template| {
                if template.is::<T>() {
                    Some(*name)
                } else {
                    None
                }
            })
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates() {
        let mut reg = DefinitionRegistryBuilder::new();
        assert!(reg.register("nice".into(), Definition::dummy()).is_ok());
        assert!(reg.register("nice".into(), Definition::dummy()).is_err()); // duplicate
        assert!(reg.register("nice2".into(), Definition::dummy()).is_ok());
    }
}
