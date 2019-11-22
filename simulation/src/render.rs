use std::cell::RefCell;
use std::rc::Rc;

use specs::prelude::*;
use specs_derive::Component;

use world::{SliceRange, ViewPoint, WorldRef};

use crate::movement::Position;

/// Physical attributes to be rendered
#[derive(Component, Debug, Copy, Clone)]
#[storage(VecStorage)]
pub struct Physical {
    /// temporary simple color
    pub color: (u8, u8, u8),

    /// 3d dimensions in world scale
    pub dimensions: (f32, f32, f32),
}

pub trait Renderer {
    type Target;

    fn init(&mut self, _target: Rc<RefCell<Self::Target>>) {}

    fn start(&mut self) {}

    fn entity(&mut self, pos: &Position, physical: &Physical);

    fn finish(&mut self) {}

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

impl<'a, R: Renderer> System<'a> for RenderSystem<'a, R> {
    type SystemData = (ReadStorage<'a, Position>, ReadStorage<'a, Physical>);

    fn run(&mut self, (pos, physical): Self::SystemData) {
        for (pos, physical) in (&pos, &physical).join() {
            if self.frame_state.slices.contains(pos.slice()) {
                self.renderer.entity(pos, physical);
            }
        }
    }
}

pub trait DebugRenderer<R: Renderer> {
    fn render(&mut self, renderer: &mut R, world: WorldRef, frame_state: &FrameRenderState<R>);
}

#[allow(dead_code)]
pub mod dummy {
    use world::{ViewPoint, WorldRef};

    use crate::render::{DebugRenderer, FrameRenderState};
    use crate::Renderer;

    pub struct DummyDebugRenderer;

    impl<R: Renderer> DebugRenderer<R> for DummyDebugRenderer {
        fn render(
            &mut self,
            renderer: &mut R,
            _world: WorldRef,
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
