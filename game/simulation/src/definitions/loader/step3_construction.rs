use std::any::Any;
use std::rc::Rc;

use common::*;

use crate::definitions::loader::step1_deserialization::{DefinitionSource, DeserializedDefinition};
use crate::definitions::loader::step2_preprocessing::ComponentFields;
use crate::definitions::loader::template_lookup::TemplateLookup;
use crate::definitions::{DefinitionError, DefinitionErrorKind, DefinitionErrors, ValueImpl};
use crate::ecs;
use crate::ecs::ComponentTemplate;
use crate::string::{CachedStr, StringCache};

#[derive(Debug)]
pub struct Definition {
    source: DefinitionSource,
    components: Vec<(CachedStr, Rc<dyn ecs::ComponentTemplate<ValueImpl>>)>,
}

pub fn instantiate(
    defs: Vec<DeserializedDefinition>,
    templates: &TemplateLookup,
    string_cache: &StringCache,
) -> Result<Vec<(CachedStr, Definition)>, DefinitionErrors> {
    let mut errors = Vec::new();

    // instantiate components
    let instantiated = defs
        .into_iter()
        .filter_map(
            |def| match Definition::construct(def, templates, string_cache) {
                Err(e) => {
                    errors.push(e);
                    None
                }
                Ok(d) => Some(d),
            },
        )
        .collect();

    if !errors.is_empty() {
        Err(DefinitionErrors(errors))
    } else {
        Ok(instantiated)
    }
}
impl Definition {
    pub fn components(&self) -> impl Iterator<Item = &dyn ComponentTemplate<ValueImpl>> {
        self.components.iter().map(|(_, c)| &**c)
    }

    pub fn find_component(&self, name: &str) -> Option<&dyn Any> {
        self.components.iter().find_map(|(comp, template)| {
            if name == comp.as_ref() {
                Some(template.as_any())
            } else {
                None
            }
        })
    }

    pub fn find_component_ref<T: 'static>(&self, name: &str) -> Option<Rc<T>> {
        self.components.iter().find_map(|(comp, template)| {
            if name == comp.as_ref() && template.as_any().is::<T>() {
                let rc = template.clone();
                // safety: type has been checked
                Some(unsafe { Rc::from_raw(Rc::into_raw(rc) as _) })
            } else {
                None
            }
        })
    }

    pub fn source(&self) -> DefinitionSource {
        self.source.clone()
    }

    fn construct(
        deserialized: DeserializedDefinition,
        templates: &TemplateLookup,
        string_cache: &StringCache,
    ) -> Result<(CachedStr, Definition), DefinitionError> {
        let (uid, source, components) = deserialized.into_inner();

        let do_construct = || {
            let mut component_templates = Vec::with_capacity(components.len());

            for (key, fields) in components.into_iter() {
                let mut map = match fields {
                    ComponentFields::Fields(fields) => ecs::Map::from_fields(fields.into_iter()),
                    ComponentFields::Unit => ecs::Map::empty(),
                    ComponentFields::Negate => unimplemented!(),
                };

                let component_template =
                    templates.construct(key.as_str(), &mut map, string_cache)?;

                for leftover in map.keys() {
                    warn!(
                        "construction of component template ignored key";
                        "template" => &key, "ignored_key" => leftover,
                    );
                }

                component_templates.push((string_cache.get(&key), component_template));
            }

            Ok((uid, component_templates))
        };

        match do_construct() {
            Ok((uid, components)) => {
                Ok((string_cache.get(&uid), Definition { source, components }))
            }
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
