use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use color::Color;
use unit::world::WorldPoint;

use crate::alloc::FrameAllocator;
use crate::ecs::*;
use crate::interact::herd::component::CurrentHerd;
use crate::interact::herd::{HerdHandle, HerdedComponent, Herds};
use crate::render::DebugRenderer;
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
        let radius = config::get().simulation.herd_radius;

        type Query<'a> = (
            Read<'a, Herds>,
            ReadStorage<'a, TransformComponent>,
            ReadStorage<'a, HerdedComponent>,
        );

        let mut visible_herds = HashSet::new();
        let (herds, transform, herd) = Query::fetch(ecs_world);
        for (transform, herd) in (&transform, &herd).join() {
            if !range.contains(transform.slice()) {
                continue;
            }

            let color = match herd.current() {
                CurrentHerd::PendingDeparture { .. } => Color::rgb(160, 160, 160),
                CurrentHerd::MemberOf(herd) => herd_color(herd),
            };

            match herd.current() {
                CurrentHerd::PendingDeparture { .. } => {
                    renderer.debug_add_circle(transform.position, radius, color);
                }
                CurrentHerd::MemberOf(_) => {
                    renderer.debug_add_circle(transform.position, 2.0, color);
                }
            };

            visible_herds.insert(herd.current().handle());

            // gets messy
            // if let Some(herd) = herds.get_info(herd.current().handle()) {
            //     renderer.debug_add_line(transform.position, herd.average_pos, color);
            // }
        }

        let alloc = ecs_world.resource::<FrameAllocator>();
        for herd in visible_herds {
            let info = match herds.get_info(herd) {
                Some(h) => h,
                None => continue,
            };

            let name = alloc.alloc_str_from_debug(&herd);
            renderer.debug_text(info.median_pos, name.as_str());
            renderer.debug_add_square_around(info.median_pos, 2.0, Color::rgb(200, 40, 20));

            const PADDING: f32 = 3.0;
            let ((ax, bx), (ay, by), (z, _)) = info.range.ranges();

            let color = herd_color(herd);
            renderer.debug_add_quad(
                [
                    WorldPoint::new_unchecked(ax - PADDING, ay - PADDING, z),
                    WorldPoint::new_unchecked(bx + PADDING, ay - PADDING, z),
                    WorldPoint::new_unchecked(bx + PADDING, by + PADDING, z),
                    WorldPoint::new_unchecked(ax - PADDING, by + PADDING, z),
                ],
                color,
            );

            let leader = ecs_world.component::<TransformComponent>(info.leader); // TODO use storage
            if let Some(leader) = info.leader.get(&transform) {
                renderer.debug_add_square_around(leader.position, 1.5, color);
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
        const SCALE: u64 = 1_000_000;
        let scaled = hashed / (u64::MAX / SCALE);
        ((scaled as f64) / SCALE as f64) as f32
    };

    Color::hsl(hue, 0.8, 0.7)
}
