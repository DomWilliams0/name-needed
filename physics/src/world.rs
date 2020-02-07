use std::os::raw::c_void;
use std::ptr::null_mut;
use std::time::Instant;

use bulletc_sys as ffi;
use common::*;
use unit::world::WorldPoint;

use crate::collider::ColliderHandle;
use crate::TICKS_PER_SECOND;

pub struct PhysicsWorld {
    dynworld: *mut ffi::dynworld,
    last_tick: Instant,
}

pub enum StepType {
    RenderOnly,
    Tick,
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
    pub fn add_entity(&mut self, pos: WorldPoint, dimensions: (f32, f32, f32)) -> ColliderHandle {
        let center: *const f32 = &[pos.0, pos.1, pos.2] as *const f32;
        let half_extents: *const f32 =
            &[dimensions.0 / 2.0, dimensions.1 / 2.0, dimensions.2 / 2.0] as *const f32;

        let collider = {
            let cfg = config::get();
            unsafe {
                ffi::entity_collider_create(
                    self.dynworld,
                    center,
                    half_extents,
                    cfg.simulation.friction,
                    cfg.simulation.linear_damping,
                    true, // TODO pointer to callback instead
                )
            }
        };

        ColliderHandle::new(collider)
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

    // pub fn handle_collision_events(&self) {}

    pub fn step(&mut self, step_type: StepType) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f32();

        const TICK_RATE: f32 = 1.0 / TICKS_PER_SECOND as f32;

        match step_type {
            StepType::RenderOnly => unsafe {
                ffi::dynworld_step_render_only(self.dynworld, elapsed)
            },
            StepType::Tick => {
                self.last_tick = now;
                unsafe { ffi::dynworld_step(self.dynworld, elapsed, TICK_RATE) }
            }
        }
    }

    pub fn sync_render_pos_from(&self, collider: &ColliderHandle) -> Option<WorldPoint> {
        let mut ffi_pos = [0.0f32; 3];

        let ret =
            unsafe { ffi::entity_collider_get_pos(collider.collider, &mut ffi_pos[0] as *mut f32) };

        if ret != 0 {
            None
        } else {
            Some(WorldPoint::from(ffi_pos))
        }
    }

    pub fn sync_from(
        &self,
        collider: &mut ColliderHandle,
        pos: &mut WorldPoint,
        rot: &mut Vector2,
    ) -> bool {
        let mut ffi_pos = [0.0f32; 3];
        let mut ffi_rot = [0.0f32; 2];
        let mut jump_sensor_occluded = false;

        let ret = unsafe {
            ffi::entity_collider_get(
                self.dynworld,
                collider.collider,
                &mut ffi_pos[0] as *mut f32,
                &mut ffi_rot[0] as *mut f32,
                &mut jump_sensor_occluded as *mut bool,
            )
        };

        if ret != 0 {
            false
        } else {
            // TODO probably kinda slow

            pos.0 = ffi_pos[0];
            pos.1 = ffi_pos[1];
            pos.2 = ffi_pos[2];

            rot.x = ffi_rot[0];
            rot.y = ffi_rot[1];

            // TODO not used yet
            // collider.set_jump_sensor_occluded(jump_sensor_occluded);

            true
        }
    }

    pub fn sync_to(
        &self,
        collider: &mut ColliderHandle,
        pos: &WorldPoint,
        rot: F,
        vel: &Vector3,
        jump_force: f32,
    ) -> bool {
        let ffi_pos: [f32; 3] = [pos.0, pos.1, pos.2];
        let ffi_vel: [f32; 3] = [vel.x, vel.y, vel.z];

        let ret = unsafe {
            ffi::entity_collider_set(
                collider.collider,
                &ffi_pos as *const f32,
                rot,
                &ffi_vel as *const f32,
                jump_force,
            )
        };

        ret == 0
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

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
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
