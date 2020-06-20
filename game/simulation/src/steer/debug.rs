use color::ColorRgb;
use world::{InnerWorldRef, SliceRange};

use crate::ecs::*;
use crate::render::DebugRenderer;
use crate::{Renderer, TransformComponent};

pub struct SteeringDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for SteeringDebugRenderer {
    fn identifier(&self) -> &'static str {
        "steering"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        ecs_world: &EcsWorld,
        slices: SliceRange,
    ) {
        type Query<'a> = (ReadStorage<'a, TransformComponent>,);
        let (transform,) = <Query as SystemData>::fetch(ecs_world);

        for (transform,) in (&transform,).join() {
            if slices.contains(transform.position.slice()) {
                let vel_pos = transform.position + (transform.velocity * 5.0);
                renderer.debug_add_line(transform.position, vel_pos, ColorRgb::new(255, 0, 50))
            }
        }
    }
}
