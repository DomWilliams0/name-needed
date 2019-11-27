use std::cell::RefCell;
use std::rc::Rc;

use world::{SliceRange, ViewPoint, WorldRef};

use crate::ecs::*;
use crate::movement::Transform;

/// Physical attributes to be rendered
#[derive(Debug, Copy, Clone)]
pub struct Physical {
    /// temporary simple color
    pub color: (u8, u8, u8),

    /// 3d dimensions in world scale
    pub dimensions: (f32, f32, f32),
}

impl Component for Physical {}

pub trait Renderer {
    type Target;

    /// Initialize frame rendering
    fn init(&mut self, _target: Rc<RefCell<Self::Target>>) {}

    /// Start rendering simulation
    fn start(&mut self) {}

    fn entity(&mut self, transform: &Transform, physical: &Physical);

    /// Finish rendering simulation
    fn finish(&mut self) {}

    /// End rendering frame
    fn deinit(&mut self) {}

    // ---

    fn debug_start(&mut self) {}

    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: (u8, u8, u8));

    fn debug_add_tri(&mut self, points: [ViewPoint; 3], color: (u8, u8, u8));

    fn debug_finish(&mut self) {}
}

//#[derive(Clone)]
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
}

impl<'a, R: Renderer> System for RenderSystem<'a, R> {
    fn tick_system(&mut self, data: &TickData) {
        data.ecs_world
            .matcher::<All<(Read<Transform>, Read<Physical>)>>()
            .for_each(|(transform, physical)| {
                if self.frame_state.slices.contains(transform.slice()) {
                    self.renderer.entity(transform, physical);
                }
            });
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
    use world::{ViewPoint, WorldRef};

    use crate::ecs::EcsWorld;
    use crate::render::{DebugRenderer, FrameRenderState};
    use crate::Renderer;

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
                (255, 0, 0),
            );
            renderer.debug_add_line(
                ViewPoint(0.0, 0.0, 0.0),
                ViewPoint(0.0, 1.0, 0.0),
                (255, 0, 0),
            );
        }
    }
}
