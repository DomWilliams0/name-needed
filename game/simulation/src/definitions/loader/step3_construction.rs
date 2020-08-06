use common::*;

use crate::definitions::loader::step1_deserialization::{
    DefinitionSource, DefinitionUid, DeserializedDefinition,
};
use crate::definitions::loader::step2_preprocessing::ComponentFields;
use crate::definitions::loader::template_lookup::TemplateLookup;
use crate::definitions::{DefinitionError, DefinitionErrorKind, DefinitionErrors, ValueImpl};
use crate::ecs;
use crate::ecs::ComponentTemplate;

#[derive(Debug)]
pub struct Definition {
    source: DefinitionSource,
    components: Vec<Box<dyn ecs::ComponentTemplate<ValueImpl>>>,
}

pub fn instantiate(
    defs: Vec<DeserializedDefinition>,
    templates: &TemplateLookup,
) -> Result<Vec<(DefinitionUid, Definition)>, DefinitionErrors> {
    let mut errors = Vec::new();

    // instantiate components
    let instantiated = defs
        .into_iter()
        .filter_map(|def| match Definition::construct(def, &templates) {
            Err(e) => {
                errors.push(e);
                None
            }
            Ok(d) => Some(d),
        })
        .collect();

    if !errors.is_empty() {
        Err(DefinitionErrors(errors))
    } else {
        Ok(instantiated)
    }
}
impl Definition {
    pub fn components(&self) -> impl Iterator<Item = &dyn ComponentTemplate<ValueImpl>> {
        self.components.iter().map(|c| &**c)
    }

    pub fn source(&self) -> DefinitionSource {
        self.source.clone()
    }

    fn construct(
        deserialized: DeserializedDefinition,
        templates: &TemplateLookup,
    ) -> Result<(DefinitionUid, Definition), DefinitionError> {
        let (uid, source, components) = deserialized.into_inner();

        let do_construct = || {
            let mut component_templates = Vec::with_capacity(components.len());

            for (key, fields) in components.into_iter() {
                let mut map = match fields {
                    ComponentFields::Fields(fields) => ecs::Map::from_fields(fields.into_iter()),
                    ComponentFields::Unit => ecs::Map::empty(),
                    ComponentFields::Negate => unimplemented!(),
                };

                let component_template = templates.construct(key.as_str(), &mut map)?;

                // bit gross to allocate a vec here just for logging
                let leftovers = map.keys().collect_vec();
                if !leftovers.is_empty() {
                    warn!(
                        "construction of component template {:?} ignored key(s): {:?}",
                        key, leftovers
                    );
                }

                component_templates.push(component_template);
            }

            Ok((uid, component_templates))
        };

        match do_construct() {
            Ok((uid, components)) => Ok((uid, Definition { source, components })),
            Err(e) => Err(DefinitionError(source, e)),
        }
    }

    pub fn make_error(&self, e: DefinitionErrorKind) -> DefinitionError {
        DefinitionError(self.source(), e)
    }

    #[cfg(test)]
    pub fn dummy() -> Self {
        Self {
            source: DefinitionSource::Memory,
            components: vec![],
        }
    }
}
