use crate::definitions::loader::step1_deserialization::{
    collect_raw_definitions, DefinitionUid, DeserializedDefinition,
};
use crate::definitions::loader::step2_preprocessing::preprocess;
use crate::definitions::loader::step3_construction::{instantiate, Definition};
use crate::definitions::loader::template_lookup::TemplateLookup;
use crate::definitions::registry::RegistryBuilder;
use crate::definitions::{DefinitionError, DefinitionErrors, Registry};

pub fn load(resources: resources::Definitions) -> Result<Registry, DefinitionErrors> {
    let defs = load_and_preprocess_with(|| collect_raw_definitions(resources))?;
    let instantiated = instantiate(defs, &TemplateLookup::init())?;
    build_registry(instantiated)
}

#[cfg(test)]
pub fn load_from_str(definitions: &str) -> Result<Registry, DefinitionErrors> {
    let defs = preprocess_from_str(definitions)?;
    let instantiated = instantiate(defs, &TemplateLookup::init())?;
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

pub fn build_registry(
    defs: Vec<(DefinitionUid, Definition)>,
) -> Result<Registry, DefinitionErrors> {
    let mut errors = Vec::new();

    let mut registry = RegistryBuilder::new();
    for (uid, definition) in defs {
        if let Err((def, err)) = registry.register(uid, definition) {
            errors.push(def.make_error(err));
        }
    }

    if !errors.is_empty() {
        Err(DefinitionErrors(errors))
    } else {
        Ok(registry.build())
    }
}
