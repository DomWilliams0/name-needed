use std::os::raw::c_void;
use std::time::Instant;

use bulletc_sys as ffi;
use unit::world::WorldPoint;

use crate::collider::Collider;
use crate::TICKS_PER_SECOND;
use std::ptr::null_mut;

pub struct PhysicsWorld {
    dynworld: *mut ffi::dynworld,
    last_tick: Instant,
}

impl PhysicsWorld {
    pub fn new(gravity: f32) -> Self {
        let _tps = 1.0f32 / TICKS_PER_SECOND as f32;
        let dynworld = unsafe { ffi::dynworld_create(gravity) };
        Self {
            dynworld,
            last_tick: Instant::now(),
        }
    }

    /// pos: center
    /// dimensions: full extents
    pub fn add_entity(&mut self, pos: WorldPoint, dimensions: (f32, f32, f32)) -> Collider {
        let center: *const f32 = &[pos.0, pos.1, pos.2] as *const f32;
        let half_extents: *const f32 =
            &[dimensions.0 / 2.0, dimensions.1 / 2.0, dimensions.2 / 2.0] as *const f32;
        let collider = unsafe { ffi::entity_collider_create(self.dynworld, center, half_extents) };
        Collider { collider }
    }

    pub fn update_slab_collider(
        &mut self,
        slab_pos: WorldPoint,
        collider: &mut SlabCollider,
        vertices: &[f32],
        indices: &[u32],
    ) {
        let old = collider.slab_collider;
        let slab_pos: [f32; 3] = slab_pos.into();
        let vertices_count = vertices.len() / 3;
        let vertices = vertices.as_ptr();
        let indices_count = indices.len();
        let indices = indices.as_ptr();

        let new = unsafe {
            ffi::slab_collider_update(
                self.dynworld,
                old,
                slab_pos.as_ptr(),
                vertices,
                vertices_count,
                indices,
                indices_count,
            )
        };
        collider.slab_collider = new;
    }

    pub fn handle_collision_events(&self) {}

    pub fn step(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        const TICK_RATE: f32 = 1.0 / TICKS_PER_SECOND as f32;

        unsafe {
            ffi::dynworld_step(self.dynworld, elapsed, TICK_RATE);
        }
    }

    pub fn position(&self, collider: &Collider) -> Option<[f32; 3]> {
        let mut pos = [0.0f32; 3];
        let ret =
            unsafe { ffi::entity_collider_position(collider.collider, &mut pos[0] as *mut f32) };
        if ret == 0 {
            Some(pos)
        } else {
            None
        }
    }

    pub fn set_debug_drawer(&mut self, enable: bool) {
        let draw_line = if enable {
            Some(debug_draw::raw_draw_line as debug_draw::FnDrawLine)
        } else {
            None
        };

        unsafe {
            ffi::dynworld_set_debug_drawer(self.dynworld, draw_line);
        }
    }

    pub fn debug_draw(&mut self, frame_blob: *mut c_void) {
        unsafe { ffi::dynworld_debug_draw(self.dynworld, frame_blob) }
    }
}

impl Drop for PhysicsWorld {
    fn drop(&mut self) {
        unsafe { ffi::dynworld_destroy(self.dynworld) };
        self.dynworld = null_mut();
    }
}

pub struct SlabCollider {
    slab_collider: *mut ffi::slab_collider,
}

impl Default for SlabCollider {
    fn default() -> Self {
        Self {
            slab_collider: null_mut(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::PhysicsWorld;

    #[test]
    fn create_and_destroy() {
        let _w = PhysicsWorld::new(0.0);
    }
}
