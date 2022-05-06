use crate::definitions::loader::step1_deserialization::{
    collect_raw_definitions, DeserializedDefinition,
};
use crate::definitions::loader::step2_preprocessing::preprocess;
use crate::definitions::loader::step3_construction::{instantiate, Definition};
use crate::definitions::loader::template_lookup::TemplateLookup;
use crate::definitions::registry::DefinitionRegistryBuilder;
use crate::definitions::{DefinitionError, DefinitionErrors, DefinitionRegistry};
use crate::string::{CachedStr, StringCache};

pub fn load(
    resources: resources::Definitions,
    string_cache: &StringCache,
) -> Result<DefinitionRegistry, DefinitionErrors> {
    let defs = load_and_preprocess_with(|| collect_raw_definitions(resources))?;
    let instantiated = instantiate(defs, &TemplateLookup::init(), string_cache)?;
    build_registry(instantiated)
}

#[cfg(test)]
pub fn load_from_str(definitions: &str) -> Result<DefinitionRegistry, DefinitionErrors> {
    let defs = preprocess_from_str(definitions)?;
    let string_cache = StringCache::default(); // cache is not cleared in tests on drop
    let instantiated = instantiate(defs, &TemplateLookup::init(), &string_cache)?;
    build_registry(instantiated)
}

#[cfg(test)]
pub fn preprocess_from_str(input: &str) -> Result<Vec<DeserializedDefinition>, DefinitionErrors> {
    load_and_preprocess_with(|| {
        match DeserializedDefinition::from_ron(
            input.as_bytes(),
            super::step1_deserialization::DefinitionSource::Memory,
        ) {
            Ok(defs) => (defs, vec![]),
            Err(e) => (vec![], vec![e]),
        }
    })
}

pub fn load_and_preprocess_with<
    F: FnOnce() -> (Vec<DeserializedDefinition>, Vec<DefinitionError>),
>(
    provider: F,
) -> Result<Vec<DeserializedDefinition>, DefinitionErrors> {
    // collect unprocessed definitions
    let (mut defs, errors) = provider();
    if !errors.is_empty() {
        return Err(DefinitionErrors(errors));
    }

    // process parent relations
    preprocess(&mut defs)?;

    // TODO remove abstract definitions

    if !errors.is_empty() {
        Err(DefinitionErrors(errors))
    } else {
        Ok(defs)
    }
}

/// defs: (uid, def, category)
pub fn build_registry(
    defs: Vec<(CachedStr, Definition, Option<String>)>,
) -> Result<DefinitionRegistry, DefinitionErrors> {
    let mut errors = Vec::new();

    let mut registry = DefinitionRegistryBuilder::new();
    for (uid, definition, category) in defs {
        if let Err((def, err)) = registry.register(uid, definition, category) {
            errors.push(def.make_error(uid.to_string(), err));
        }
    }

    if !errors.is_empty() {
        Err(DefinitionErrors(errors))
    } else {
        Ok(registry.build())
    }
}
