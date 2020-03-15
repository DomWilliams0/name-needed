use std::cell::RefCell;
use std::rc::Rc;

use color::ColorRgb;
use physics::StepType;
use world::{SliceRange, WorldRef};

use crate::ecs::*;
use crate::physics::PhysicsComponent;
use crate::TransformComponent;
use unit::view::ViewPoint;

/// Physical attributes to be rendered
#[derive(Debug, Copy, Clone)]
pub struct PhysicalComponent {
    /// temporary simple color
    pub color: ColorRgb,

    /// 3d dimensions in world scale
    pub dimensions: (f32, f32, f32),
}

pub trait Renderer {
    type Target;

    /// Initialize frame rendering
    fn init(&mut self, _target: Rc<RefCell<Self::Target>>) {}

    /// Start rendering simulation
    fn start(&mut self) {}

    fn entity(&mut self, transform: &TransformComponent, physical: &PhysicalComponent);

    /// Finish rendering simulation
    fn finish(&mut self) {}

    /// End rendering frame
    fn deinit(&mut self) {}

    // ---

    fn debug_start(&mut self) {}

    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb);

    fn debug_add_tri(&mut self, points: [ViewPoint; 3], color: ColorRgb);

    fn debug_finish(&mut self) {}
}

pub struct FrameRenderState<R: Renderer> {
    pub target: Rc<RefCell<R::Target>>,
    pub slices: SliceRange,
}

impl<R: Renderer> Clone for FrameRenderState<R> {
    fn clone(&self) -> Self {
        Self {
            target: self.target.clone(),
            slices: self.slices,
        }
    }
}

/// Wrapper for calling generic Renderer in render system
pub(crate) struct RenderSystem<'a, R: Renderer> {
    pub renderer: &'a mut R,
    pub frame_state: FrameRenderState<R>,
    pub interpolation: f64,
}

impl<'a, R: Renderer> System for RenderSystem<'a, R> {
    fn tick_system(&mut self, data: &mut TickData) {
        let mut voxel_world = data.voxel_world.borrow_mut();
        let physics_world = voxel_world.physics_world_mut();

        physics_world.step(StepType::RenderOnly);

        let query = <(
            Read<TransformComponent>,
            Read<PhysicalComponent>,
            TryRead<PhysicsComponent>,
        )>::query();
        for (transform, physical, physics) in query.iter(data.ecs_world) {
            if self.frame_state.slices.contains(transform.slice()) {
                // make copy to mutate for interpolation
                let mut transform = *transform;

                if let Some(physics) = physics {
                    if let Some(interpolated_pos) =
                        physics_world.sync_render_pos_from(&physics.collider)
                    {
                        transform.position = interpolated_pos;
                    }
                }

                self.renderer.entity(&transform, physical.as_ref());
            }
        }
    }
}

pub trait DebugRenderer<R: Renderer> {
    fn render(
        &mut self,
        renderer: &mut R,
        world: WorldRef,
        ecs_world: &EcsWorld,
        frame_state: &FrameRenderState<R>,
    );
}

#[allow(dead_code)]
pub mod dummy {
    use color::ColorRgb;
    use world::WorldRef;

    use crate::ecs::EcsWorld;
    use crate::render::{DebugRenderer, FrameRenderState};
    use crate::Renderer;
    use unit::view::ViewPoint;

    /// Example renderer that draws lines at the origin along the X and Y axes
    pub struct DummyDebugRenderer;

    impl<R: Renderer> DebugRenderer<R> for DummyDebugRenderer {
        fn render(
            &mut self,
            renderer: &mut R,
            _world: WorldRef,
            _ecs_world: &EcsWorld,
            _frame_state: &FrameRenderState<R>,
        ) {
            renderer.debug_add_line(
                ViewPoint(0.0, 0.0, 0.0),
                ViewPoint(1.0, 0.0, 0.0),
                ColorRgb::new(255, 0, 0),
            );
            renderer.debug_add_line(
                ViewPoint(0.0, 0.0, 0.0),
                ViewPoint(0.0, 1.0, 0.0),
                ColorRgb::new(255, 0, 0),
            );
        }
    }
}
