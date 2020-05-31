use serde::Serialize;

/// Only valid within a Span::Entity
#[derive(Copy, Clone, PartialEq, Debug, Serialize)]
pub enum EntityEvent {
    /// Entity has been created
    Create,

    /// Entity is going to navigate to this position
    NewNavigationTarget((f32,f32,f32)),

    /// Entity reached its target
    NavigationTargetReached,
}
