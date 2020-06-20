use world::{InnerWorldRef, SliceRange};

use crate::ecs::*;
use crate::path::FollowPathComponent;
use crate::render::DebugRenderer;
use crate::{RenderComponent, Renderer, TransformComponent};
use unit::world::WorldPoint;

pub struct PathDebugRenderer {
    waypoints: Vec<WorldPoint>,
}

impl Default for PathDebugRenderer {
    fn default() -> Self {
        Self {
            waypoints: Vec::with_capacity(128),
        }
    }
}

impl<R: Renderer> DebugRenderer<R> for PathDebugRenderer {
    fn identifier(&self) -> &'static str {
        "navigation path"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        ecs_world: &EcsWorld,
        slices: SliceRange,
    ) {
        type Query<'a> = (
            ReadStorage<'a, FollowPathComponent>,
            ReadStorage<'a, TransformComponent>,
            ReadStorage<'a, RenderComponent>,
        );

        let (path, transform, render) = <Query as SystemData>::fetch(ecs_world);

        for (follow_path, transform, render) in (&path, &transform, &render).join() {
            if !slices.contains(transform.slice()) {
                continue;
            }

            follow_path.waypoints(&mut self.waypoints);
            if self.waypoints.is_empty() {
                continue;
            }

            let mut line_from = transform.position;
            for line_to in self.waypoints.iter().copied() {
                renderer.debug_add_line(line_from, line_to, render.color());

                line_from = line_to;
            }

            self.waypoints.clear();
        }
    }
}
