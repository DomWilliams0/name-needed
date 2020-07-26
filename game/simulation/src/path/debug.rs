use world::{BaseTerrain, InnerWorldRef, WorldViewer};

use crate::ecs::*;
use crate::path::FollowPathComponent;
use crate::render::DebugRenderer;
use crate::{RenderComponent, Renderer, TransformComponent};
use color::ColorRgb;

use std::hash::Hasher;

use unit::world::{GlobalSliceIndex, WorldPoint};

pub struct PathDebugRenderer {
    waypoints: Vec<WorldPoint>,
}

#[derive(Default)]
pub struct NavigationAreaDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for PathDebugRenderer {
    fn identifier(&self) -> &'static str {
        "navigation path"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        ecs_world: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        type Query<'a> = (
            ReadStorage<'a, FollowPathComponent>,
            ReadStorage<'a, TransformComponent>,
            ReadStorage<'a, RenderComponent>,
        );

        let (path, transform, render) = <Query as SystemData>::fetch(ecs_world);
        let slices = viewer.entity_range();

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

impl<R: Renderer> DebugRenderer<R> for NavigationAreaDebugRenderer {
    fn identifier(&self) -> &'static str {
        "navigation areas"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        world: &InnerWorldRef,
        _: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        // TODO only render the top area in each slice
        let slices = viewer.entity_range();
        for visible_chunk in viewer.visible_chunks() {
            if let Some(chunk) = world.find_chunk_with_pos(visible_chunk) {
                for slice_idx in slices.as_range().rev() {
                    if let Some(slice) = chunk.slice(slice_idx) {
                        for (pos, block) in slice.blocks() {
                            if let Some(slab_area) = block.walkable_area() {
                                // generate a random hue from the chunk & slab area
                                let unique_color = {
                                    let unique_id = chunk.id().wrapping_add(slab_area.0 as u64);

                                    // hash for more even distribution
                                    let mut hasher = ahash::AHasher::new_with_keys(1, 2);
                                    hasher.write_u64(unique_id);
                                    let hashed = hasher.finish();

                                    // scale down from ridiculous u64 range
                                    const SCALE: u64 = 1_000_000;
                                    let scaled = hashed / (u64::MAX / SCALE);
                                    ((scaled as f64) / SCALE as f64) as f32
                                };

                                let color = ColorRgb::new_hsl(unique_color, 0.8, 0.7);
                                let pos = pos
                                    .to_block_position(GlobalSliceIndex::new(slice_idx))
                                    .to_world_point_centered(chunk.pos());
                                renderer.debug_add_square_around(pos, 0.1, color);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for PathDebugRenderer {
    fn default() -> Self {
        Self {
            waypoints: Vec::with_capacity(128),
        }
    }
}
