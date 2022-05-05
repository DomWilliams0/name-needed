use std::collections::HashMap;

use common::*;

use crate::definitions::builder::DefinitionBuilder;
use crate::definitions::{Definition, DefinitionErrorKind};
use crate::string::{CachedStr, CachedStringHasher, StringCache};
use crate::ComponentWorld;

pub struct DefinitionRegistry {
    definitions: HashMap<CachedStr, Definition, CachedStringHasher>,
    categories: HashMap<String, Vec<CachedStr>>,
}

pub struct DefinitionRegistryBuilder {
    definitions: HashMap<CachedStr, Definition, CachedStringHasher>,
    categories: HashMap<String, Vec<CachedStr>>,
}

impl DefinitionRegistryBuilder {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::with_capacity_and_hasher(512, CachedStringHasher::default()),
            categories: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        uid: CachedStr,
        definition: Definition,
        category: Option<String>,
    ) -> Result<(), (Definition, DefinitionErrorKind)> {
        #[allow(clippy::map_entry)]
        if self.definitions.contains_key(&uid) {
            Err((
                definition,
                DefinitionErrorKind::AlreadyRegistered(uid.as_ref().to_owned()),
            ))
        } else {
            self.definitions.insert(uid, definition);

            if let Some(category) = category {
                self.categories
                    .entry(category)
                    .and_modify(|defs| defs.push(uid))
                    .or_insert_with(|| vec![uid]);
            }
            Ok(())
        }
    }

    pub fn build(self) -> DefinitionRegistry {
        info!(
            "creating definition registry with {count} entries",
            count = self.definitions.len()
        );
        DefinitionRegistry {
            definitions: self.definitions,
            categories: self.categories,
        }
    }
}

impl DefinitionRegistry {
    pub fn instantiate<'s, 'w: 's, W: ComponentWorld>(
        &'s self,
        uid: &str,
        world: &'w W,
    ) -> Result<DefinitionBuilder<'s, W>, DefinitionErrorKind> {
        let uid = world.resource::<StringCache>().get(uid);
        match self.definitions.get(&uid) {
            Some(def) => Ok(DefinitionBuilder::new_with_cached(def, world, uid)),
            None => Err(DefinitionErrorKind::NoSuchDefinition(
                uid.as_ref().to_owned(),
            )),
        }
    }

    pub fn lookup_definition(&self, uid: CachedStr) -> Option<&Definition> {
        self.definitions.get(&uid)
    }

    pub fn iter_category(
        &self,
        category: &str,
    ) -> impl Iterator<Item = (CachedStr, &Definition)> + '_ {
        self.categories
            .get(category)
            .map(|defs| defs.iter().copied())
            .into_iter()
            .flatten()
            .map(move |uid| match self.definitions.get(&uid) {
                Some(def) => (uid, def),
                None => panic!(
                    "expected definition '{uid}' to exist since it's registered under a category"
                ),
            })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates() {
        let mut reg = DefinitionRegistryBuilder::new();
        assert!(reg
            .register("nice".into(), Definition::dummy(), None)
            .is_ok());
        assert!(reg
            .register("nice".into(), Definition::dummy(), None)
            .is_err()); // duplicate
        assert!(reg
            .register("nice2".into(), Definition::dummy(), None)
            .is_ok());
    }
}
