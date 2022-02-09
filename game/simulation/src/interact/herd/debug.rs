use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use color::Color;
use common::Point3;
use unit::world::WorldPoint;

use crate::ecs::*;

use crate::interact::herd::{HerdHandle, HerdedComponent};
use crate::render::DebugRenderer;

use crate::alloc::FrameAllocator;
use crate::{InnerWorldRef, Renderer, ThreadedWorldLoader, TransformComponent, WorldViewer};

#[derive(Default)]
pub struct HerdDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for HerdDebugRenderer {
    fn name(&self) -> &'static str {
        "Herds"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        ecs_world: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        let range = viewer.entity_range();
        // let radius = config::get().simulation.herd_radius;

        type Query<'a> = (
            ReadStorage<'a, TransformComponent>,
            ReadStorage<'a, HerdedComponent>,
        );

        // const RADIUS_COLOR: Color = Color::rgb(92, 211, 247);

        // TODO this will be pre calculated already
        let mut total_herd = HashMap::new();

        struct HerdEntry {
            members: usize,
            summed_pos: Point3,
            min_pos: Point3,
            max_pos: Point3,
        }

        let (transform, herd) = Query::fetch(ecs_world);
        for (transform, herd) in (&transform, &herd).join() {
            if !range.contains(transform.slice()) {
                continue;
            }

            // let color = herd_color(herd.handle());
            // renderer.debug_add_circle(transform.position, radius, color);

            total_herd
                .entry(herd.handle())
                .and_modify(|entry: &mut HerdEntry| {
                    let pos = transform.position.into();
                    entry.members += 1;
                    entry.summed_pos += pos;
                    entry.min_pos = Point3::new(
                        entry.min_pos.x.min(pos.x),
                        entry.min_pos.y.min(pos.y),
                        entry.min_pos.z.min(pos.z),
                    );
                    entry.max_pos = Point3::new(
                        entry.max_pos.x.max(pos.x),
                        entry.max_pos.y.max(pos.y),
                        entry.max_pos.z.max(pos.z),
                    );
                })
                .or_insert_with(|| {
                    let pos = transform.position.into();
                    HerdEntry {
                        members: 1,
                        summed_pos: pos,
                        min_pos: pos,
                        max_pos: pos,
                    }
                });
        }

        let alloc = ecs_world.resource::<FrameAllocator>();
        for (herd, entry) in total_herd {
            debug_assert_ne!(entry.members, 0);
            let n = entry.members as f32;
            let avg_pos = WorldPoint::new_unchecked(
                entry.summed_pos.x / n,
                entry.summed_pos.y / n,
                entry.summed_pos.z / n,
            );

            let name = alloc.alloc_str_from_debug(&herd);
            renderer.debug_text(avg_pos, name.as_str());
            renderer.debug_add_square_around(avg_pos, 2.0, Color::rgb(200, 40, 20));

            let min = WorldPoint::new(entry.min_pos.x, entry.min_pos.y, entry.min_pos.z);
            let max = WorldPoint::new(entry.max_pos.x, entry.max_pos.y, entry.max_pos.z);
            if let Some((min, max)) = min.zip(max) {
                const PADDING: f32 = 3.0;

                let min = min + (-PADDING, -PADDING, 0.0);
                let max = max + (PADDING, PADDING, 0.0);

                let w = max.x() - min.x();
                let h = max.y() - min.y();
                renderer.debug_add_quad(
                    [
                        min,
                        min + (w, 0.0, 0.0),
                        min + (w, h, 0.0),
                        min + (0.0, h, 0.0),
                    ],
                    herd_color(herd),
                );
            }
        }
    }
}

fn herd_color(herd: HerdHandle) -> Color {
    let hue = {
        let mut hasher = ahash::AHasher::new_with_keys(1, 2);
        herd.hash(&mut hasher);
        let hashed = hasher.finish();

        // scale down from ridiculous u64 range
        const SCALE: u64 = 1_000;
        let scaled = hashed / (u64::MAX / SCALE);
        ((scaled as f64) / SCALE as f64) as f32
    };

    Color::hsl(hue, 0.8, 0.7)
}
