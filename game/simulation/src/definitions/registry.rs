use crate::definitions::builder::DefinitionBuilder;
use crate::definitions::{Definition, DefinitionErrorKind};

use crate::definitions::loader::DefinitionUid;
use crate::ComponentWorld;
use common::*;
use std::collections::HashMap;

pub struct Registry {
    map: HashMap<String, Definition>,
}

pub struct RegistryBuilder {
    map: HashMap<String, Definition>,
}

impl RegistryBuilder {
    pub fn new() -> Self {
        Self {
            map: HashMap::with_capacity(512),
        }
    }

    pub fn register(
        &mut self,
        uid: DefinitionUid,
        definition: Definition,
    ) -> Result<(), (Definition, DefinitionErrorKind)> {
        #[allow(clippy::map_entry)]
        if self.map.contains_key(&uid) {
            Err((definition, DefinitionErrorKind::AlreadyRegistered(uid)))
        } else {
            self.map.insert(uid, definition);
            Ok(())
        }
    }

    pub fn build(self) -> Registry {
        my_info!(
            "creating definition registry with {count} entries",
            count = self.map.len()
        );
        Registry { map: self.map }
    }
}

impl Registry {
    pub fn instantiate<'s, 'w: 's, W: ComponentWorld>(
        &'s self,
        uid: &str,
        world: &'w mut W,
    ) -> Result<DefinitionBuilder<W>, DefinitionErrorKind> {
        match self.map.get(uid) {
            Some(def) => Ok(DefinitionBuilder::new(def, world)),
            None => Err(DefinitionErrorKind::NoSuchDefinition(uid.to_owned())),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates() {
        let mut reg = RegistryBuilder::new();

        assert!(reg.register("nice".to_owned(), Definition::dummy()).is_ok());
        assert!(reg
            .register("nice".to_owned(), Definition::dummy())
            .is_err()); // duplicate
        assert!(reg
            .register("nice2".to_owned(), Definition::dummy())
            .is_ok());
    }
}
