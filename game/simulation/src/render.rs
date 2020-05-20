use color::ColorRgb;
use common::{Vector3, VectorSpace};
use unit::view::ViewPoint;
use world::{SliceRange, WorldRef};

use crate::ecs::*;
use crate::TransformComponent;
use std::fmt::Debug;

/// Physical attributes to be rendered
#[derive(Debug, Copy, Clone)]
pub struct PhysicalComponent {
    /// temporary simple color
    color: ColorRgb,

    /// simple circle with diameter in x + y dims
    radius: f32,
}

impl PhysicalComponent {
    pub fn new(color: ColorRgb, radius: f32) -> Self {
        // TODO result
        assert!(radius > 0.2);

        Self { color, radius }
    }

    pub fn color(self) -> ColorRgb {
        self.color
    }
    pub fn radius(self) -> f32 {
        self.radius
    }
}

impl Component for PhysicalComponent {
    type Storage = VecStorage<Self>;
}

pub trait Renderer {
    type Target;
    type Error: Debug;

    /// Initialize frame rendering
    fn init(&mut self, target: Self::Target);

    /// Start rendering simulation
    fn sim_start(&mut self);

    /// `transform` is interpolated
    fn sim_entity(&mut self, transform: &TransformComponent, physical: PhysicalComponent);

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
        ReadStorage<'a, PhysicalComponent>,
    );

    fn run(&mut self, (transform, physical): Self::SystemData) {
        for (transform, physical) in (&transform, &physical).join() {
            if self.slices.contains(transform.slice()) {
                // make copy to mutate for interpolation
                let mut transform = *transform;

                transform.position = {
                    let last_pos: Vector3 = transform.last_position.into();
                    let curr_pos: Vector3 = transform.position.into();
                    last_pos.lerp(curr_pos, self.interpolation).into()
                };

                self.renderer.sim_entity(&transform, *physical);
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
