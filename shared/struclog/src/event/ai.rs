use serde::{Serialize, Serializer};
use std::sync::Arc;

/// Only valid within a Span::Entity
#[derive(Clone, Serialize)]
pub enum AiEvent {
    Consideration(#[serde(serialize_with = "arc")] Arc<String>, f32),
}

fn arc<S: Serializer>(arc: &Arc<String>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(arc.as_str())
}
