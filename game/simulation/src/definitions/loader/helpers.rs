use serde::de::Error;

use crate::definitions::DefinitionErrorKind;

pub fn extract_map(value: ron::Value) -> Result<ron::Map, DefinitionErrorKind> {
    match value {
        ron::Value::Map(map) => Ok(map),
        _ => Err(DefinitionErrorKind::Format(ron::Error::custom("not a map"))),
    }
}

pub fn extract_string_ref(value: &ron::Value) -> Result<&str, DefinitionErrorKind> {
    match value {
        ron::Value::String(string) => Ok(string),
        _ => Err(DefinitionErrorKind::Format(ron::Error::custom(
            "not a string",
        ))),
    }
}
