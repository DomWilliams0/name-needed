use crate::definitions::builder::DefinitionBuilder;
use crate::definitions::{Definition, DefinitionErrorKind};
use std::any::Any;

use crate::definitions::loader::DefinitionUid;
use crate::ComponentWorld;
use common::*;
use std::collections::HashMap;

pub struct DefinitionRegistry(HashMap<String, Definition>);

pub struct DefinitionRegistryBuilder(HashMap<String, Definition>);

impl DefinitionRegistryBuilder {
    pub fn new() -> Self {
        Self(HashMap::with_capacity(512))
    }

    pub fn register(
        &mut self,
        uid: DefinitionUid,
        definition: Definition,
    ) -> Result<(), (Definition, DefinitionErrorKind)> {
        #[allow(clippy::map_entry)]
        if self.0.contains_key(&uid) {
            Err((definition, DefinitionErrorKind::AlreadyRegistered(uid)))
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
        match self.0.get(uid) {
            Some(def) => Ok(DefinitionBuilder::new(def, world, uid)),
            None => Err(DefinitionErrorKind::NoSuchDefinition(uid.to_owned())),
        }
    }

    pub fn lookup_template(&self, uid: &str, component: &str) -> Option<&dyn Any> {
        self.0
            .get(uid)
            .and_then(|def| def.find_component(component))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates() {
        let mut reg = DefinitionRegistryBuilder::new();

        assert!(reg.register("nice".to_owned(), Definition::dummy()).is_ok());
        assert!(reg
            .register("nice".to_owned(), Definition::dummy())
            .is_err()); // duplicate
        assert!(reg
            .register("nice2".to_owned(), Definition::dummy())
            .is_ok());
    }
}
