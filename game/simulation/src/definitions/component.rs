use crate::ecs::*;
use crate::string::CachedStr;

/// Terrible, inefficient and a disgusting way of identifying entities by their original definition
/// name. This exists only for initial hacky entity comparisons for build job requirements, which
/// will later depend on a flexible list of properties per entity instead of their specific
/// definition name.
/// TODO remove knowledge of original definition name from entities
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("original-definition")]
pub struct DefinitionNameComponent(pub CachedStr);
