use unit::view::ViewPoint;
use unit::world::WorldPoint;
use world::WorldRef;

use crate::ecs::*;
use crate::path::FollowPath;
use crate::render::{DebugRenderer, FrameRenderState};
use crate::{Physical, Renderer, Transform};

pub struct PathDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for PathDebugRenderer {
    fn render(
        &mut self,
        renderer: &mut R,
        _world: WorldRef,
        ecs_world: &EcsWorld,
        _frame_state: &FrameRenderState<R>,
    ) {
        ecs_world
            .matcher_with_entities::<All<(Read<FollowPath>, Read<Transform>)>>()
            .for_each(|(e, (follow_path, transform))| {
                if let Some(path) = follow_path.path() {
                    if let Some(Physical { color, .. }) = ecs_world.get_component::<Physical>(e) {
                        let mut line_from = transform.position;
                        for (next_point, _) in path.iter() {
                            let line_to = WorldPoint::from(*next_point);
                            renderer.debug_add_line(
                                ViewPoint::from(line_from),
                                ViewPoint::from(line_to),
                                *color,
                            );

                            line_from = line_to;
                        }
                    }
                }
            });
    }
}
