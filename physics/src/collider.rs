use bulletc_sys as ffi;
use crate::F;

// TODO more context
#[derive(Copy, Clone, Debug)]
pub enum ColliderData {
    World,
    Entity,
}

// TODO separate body struct
// TODO collider handle
pub struct Collider {
    pub(crate) collider: *mut ffi::entity_collider,
}

// will not be used between threads so this is to allow a pointer in a component
unsafe impl Sync for Collider {}
unsafe impl Send for Collider {}

impl Collider {}
