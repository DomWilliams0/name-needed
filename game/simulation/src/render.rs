use color::ColorRgb;
use unit::view::ViewPoint;
use world::{SliceRange, WorldRef};

use crate::ecs::*;
use crate::TransformComponent;
use common::{Vector3, VectorSpace};

/// Physical attributes to be rendered
#[derive(Debug, Copy, Clone)]
pub struct PhysicalComponent {
    /// temporary simple color
    color: ColorRgb,

    /// simple circle with diameter in x + y dims
    diameter: f32,

    /// height in z axis
    height: f32,
}

impl PhysicalComponent {
    pub fn new(color: ColorRgb, diameter: f32, height: f32) -> Self {
        // TODO result
        assert!(diameter > 0.2);
        assert!(height > 0.2);

        Self {
            color,
            diameter,
            height,
        }
    }

    pub fn color(&self) -> ColorRgb {
        self.color
    }
    pub fn radius(&self) -> f32 {
        self.diameter
    }
    pub fn height(&self) -> f32 {
        self.height
    }
}

impl Component for PhysicalComponent {
    type Storage = VecStorage<Self>;
}

pub trait Renderer {
    type Target;

    /// Initialize frame rendering
    fn init(&mut self, target: Self::Target);

    /// Start rendering simulation
    fn sim_start(&mut self);

    /// `transform` is interpolated
    fn sim_entity(&mut self, transform: &TransformComponent, physical: &PhysicalComponent);

    /// Finish rendering simulation
    fn sim_finish(&mut self);

    /// End rendering frame
    fn deinit(&mut self) -> Self::Target;

    // ---

    fn debug_start(&mut self) {}

    fn debug_add_line(&mut self, _from: ViewPoint, _to: ViewPoint, _color: ColorRgb) {}

    fn debug_add_tri(&mut self, _points: [ViewPoint; 3], _color: ColorRgb) {}

    fn debug_finish(&mut self) {}
}

pub struct FrameRenderState<'t, R: Renderer> {
    pub target: &'t mut R::Target,
    pub slices: SliceRange,
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

                self.renderer.sim_entity(&transform, &physical);
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
    use unit::view::ViewPoint;
    use world::WorldRef;

    use crate::ecs::EcsWorld;
    use crate::render::{DebugRenderer, FrameRenderState};
    use crate::Renderer;

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
