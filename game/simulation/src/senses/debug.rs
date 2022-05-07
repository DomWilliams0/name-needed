use crate::ecs::*;
use crate::input::SelectedEntities;
use crate::render::DebugRenderer;

use crate::senses::SensesComponent;
use crate::{InnerWorldRef, Renderer, ThreadedWorldLoader, TransformComponent, WorldViewer};
use color::Color;
use common::cgmath::Rotation;
use common::*;

const COLOR_VISION: Color = Color::rgb(70, 200, 100);
const COLOR_HEARING: Color = Color::rgb(180, 200, 80);

#[derive(Default)]
pub struct SensesDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for SensesDebugRenderer {
    fn name(&self) -> &'static str {
        "Senses"
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
        for &selected in ecs_world.resource::<SelectedEntities>().iter() {
            let transform = ecs_world.component::<TransformComponent>(selected);
            let senses = ecs_world.component::<SensesComponent>(selected);

            if let Some((transform, senses)) = transform.ok().zip(senses.ok()) {
                if viewer.entity_range().contains(transform.slice()) {
                    let forward = transform.forwards();

                    for vision in &senses.vision {
                        let vision_fwd = forward * vision.length;
                        let rot_a = Basis2::from_angle(vision.angle_offset + (vision.angle * 0.5));
                        let rot_b = Basis2::from_angle(vision.angle_offset + (vision.angle * -0.5));

                        renderer.debug_add_line(
                            transform.position,
                            transform.position + rot_a.rotate_vector(vision_fwd),
                            COLOR_VISION,
                        );

                        renderer.debug_add_line(
                            transform.position,
                            transform.position + rot_b.rotate_vector(vision_fwd),
                            COLOR_VISION,
                        );
                    }

                    for hearing in &senses.hearing {
                        renderer.debug_add_circle(
                            transform.position,
                            hearing.radius,
                            COLOR_HEARING,
                        );
                    }

                    for entity in senses.sensed_entities() {
                        if let Ok(transform) = ecs_world.component::<TransformComponent>(entity) {
                            if range.contains(transform.slice()) {
                                renderer.debug_add_circle(transform.position, 1.0, COLOR_VISION);
                            }
                        }
                    }
                }
            }
        }
    }
}
