use std::ops::BitOrAssign;

use bulletc_sys as ffi;

// TODO more context
#[derive(Copy, Clone, Debug)]
pub enum ColliderData {
    WorldTerrain,
    Entity,
}

// TODO separate body struct
// TODO collider handle
pub struct ColliderHandle {
    pub(crate) collider: *mut ffi::entity_collider,

    /// TODO will be replaced in #75
    jump_sensor_occluded: Option<u8>,
}

// will not be used between threads so this is to allow a pointer in a component
unsafe impl Sync for ColliderHandle {}

unsafe impl Send for ColliderHandle {}

impl ColliderHandle {
    pub fn new(collider: *mut ffi::entity_collider) -> Self {
        Self {
            collider,
            jump_sensor_occluded: Some(0),
        }
    }
    /// Pops old value
    pub fn jump_sensor_occluded(&mut self) -> bool {
        self.jump_sensor_occluded.replace(0).unwrap_or(0) == 1
    }

    pub fn set_jump_sensor_occluded(&mut self, occluded: bool) {
        if let Some(val) = self.jump_sensor_occluded.as_mut() {
            // persistently set to 1/true until cleared
            val.bitor_assign(occluded as u8);
        }
    }
}
