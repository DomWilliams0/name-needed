use color::Color;

use crate::ecs::*;
use crate::render::DebugRenderer;
use crate::{InnerWorldRef, Renderer, ThreadedWorldLoader, TransformComponent, WorldViewer};

// TODO show actual steering direction alongside velocity
#[derive(Default)]
pub struct SteeringDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for SteeringDebugRenderer {
    fn identifier(&self) -> &'static str {
        "steering"
    }

    fn name(&self) -> &'static str {
        "Steering\0"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        ecs_world: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        type Query<'a> = (ReadStorage<'a, TransformComponent>,);
        let (transform,) = <Query as SystemData>::fetch(ecs_world);
        let slices = viewer.entity_range();

        for (transform,) in (&transform,).join() {
            if slices.contains(transform.position.slice()) {
                let vel_pos = transform.position + (transform.velocity * 10.0);
                renderer.debug_add_line(transform.position, vel_pos, Color::rgb(255, 0, 50))
            }
        }
    }
}
