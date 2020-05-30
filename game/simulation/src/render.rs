use color::ColorRgb;
use common::{Vector3, VectorSpace};
use unit::view::ViewPoint;
use world::{SliceRange, WorldRef};

use crate::ecs::*;
use crate::TransformComponent;
use std::fmt::Debug;

#[derive(Debug, Copy, Clone)]
pub enum PhysicalShape {
    /// Ordinal 0
    Circle { radius: f32 },
    /// Ordinal 1
    Rectangle { rx: f32, ry: f32 },
}

impl PhysicalShape {
    /// For simple sorting
    pub fn ord(self) -> usize {
        match self {
            PhysicalShape::Circle { .. } => 0,
            PhysicalShape::Rectangle { .. } => 1,
        }
    }

    pub fn circle(radius: f32) -> Self {
        PhysicalShape::Circle { radius }
    }

    pub fn rect(rx: f32, ry: f32) -> Self {
        PhysicalShape::Rectangle { rx, ry }
    }

    pub fn square(r: f32) -> Self {
        PhysicalShape::Rectangle { rx: r, ry: r }
    }

    pub fn radius(&self) -> f32 {
        match self {
            PhysicalShape::Circle { radius } => *radius,
            PhysicalShape::Rectangle { rx, ry } => rx.max(*ry),
        }
    }
}

#[derive(Debug, Clone, Component)]
#[storage(VecStorage)]
pub struct RenderComponent {
    /// simple color
    color: ColorRgb,

    /// simple 2D shape
    shape: PhysicalShape,
}

impl RenderComponent {
    pub fn new(color: ColorRgb, shape: PhysicalShape) -> Self {
        Self { color, shape }
    }

    pub fn color(&self) -> ColorRgb {
        self.color
    }
    pub fn shape(&self) -> PhysicalShape {
        self.shape
    }
}

pub trait Renderer {
    type Target;
    type Error: Debug;

    /// Initialize frame rendering
    fn init(&mut self, target: Self::Target);

    /// Start rendering simulation
    fn sim_start(&mut self);

    /// `transform` is interpolated
    fn sim_entity(&mut self, transform: &TransformComponent, render: &RenderComponent);

    /// Finish rendering simulation
    fn sim_finish(&mut self) -> Result<(), Self::Error>;

    fn debug_start(&mut self) {}

    #[allow(unused_variables)]
    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb) {}

    #[allow(unused_variables)]
    fn debug_add_tri(&mut self, points: [ViewPoint; 3], color: ColorRgb) {}

    fn debug_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// End rendering frame
    fn deinit(&mut self) -> Self::Target;
}

/// Wrapper for calling generic Renderer in render system
pub(crate) struct RenderSystem<'a, R: Renderer> {
    pub renderer: &'a mut R,
    pub slices: SliceRange,
    pub interpolation: f32,
}

impl<'a, R: Renderer> System<'a> for RenderSystem<'a, R> {
    type SystemData = (
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, RenderComponent>,
    );

    fn run(&mut self, (transform, render): Self::SystemData) {
        for (transform, render) in (&transform, &render).join() {
            if self.slices.contains(transform.slice()) {
                // make copy to mutate for interpolation
                let mut transform = *transform;

                transform.position = {
                    let last_pos: Vector3 = transform.last_position.into();
                    let curr_pos: Vector3 = transform.position.into();
                    last_pos.lerp(curr_pos, self.interpolation).into()
                };

                self.renderer.sim_entity(&transform, render);
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
        slices: SliceRange,
    );
}

pub mod dummy {
    use color::ColorRgb;
    use unit::view::ViewPoint;
    use world::{SliceRange, WorldRef};

    use crate::ecs::EcsWorld;
    use crate::render::DebugRenderer;
    use crate::Renderer;

    /// Example renderer that draws lines at the origin along the X and Y axes
    pub struct AxesDebugRenderer;

    impl<R: Renderer> DebugRenderer<R> for AxesDebugRenderer {
        fn render(&mut self, renderer: &mut R, _: WorldRef, _: &EcsWorld, _: SliceRange) {
            renderer.debug_add_line(
                ViewPoint(0.0, 0.0, 1.0),
                ViewPoint(1.0, 0.0, 1.0),
                ColorRgb::new(255, 0, 0),
            );
            renderer.debug_add_line(
                ViewPoint(0.0, 0.0, 1.0),
                ViewPoint(0.0, 1.0, 1.0),
                ColorRgb::new(0, 255, 0),
            );
        }
    }
}
