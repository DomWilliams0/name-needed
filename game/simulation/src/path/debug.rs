use world::{SliceRange, WorldRef};

use crate::ecs::*;
use crate::render::DebugRenderer;
use crate::Renderer;

pub struct PathDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for PathDebugRenderer {
    fn render(
        &mut self,
        _renderer: &mut R,
        _world: WorldRef,
        _ecs_world: &EcsWorld,
        _slices: SliceRange,
    ) {
        // TODO debug path renderer
        // let query = <(
        //     Read<FollowPathComponent>,
        //     Read<TransformComponent>,
        //     Read<PhysicalComponent>,
        // )>::query();
        // for (follow_path, transform, physical) in query.iter_immutable(ecs_world) {
        //     if let Some(path) = follow_path.path() {
        //         let mut line_from = transform.position;
        //         for (next_point, _) in path.iter() {
        //             let line_to = WorldPoint::from(*next_point);
        //             renderer.debug_add_line(
        //                 ViewPoint::from(line_from),
        //                 ViewPoint::from(line_to),
        //                 physical.color,
        //             );
        //
        //             line_from = line_to;
        //         }
        //     }
        // }
    }
}
