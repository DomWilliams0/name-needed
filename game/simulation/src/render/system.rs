use crate::ecs::*;
use crate::input::SelectedComponent;
use crate::render::renderer::Renderer;
use crate::render::shape::PhysicalShape;
use crate::{SliceRange, TransformComponent};
use color::ColorRgb;
use common::*;
use std::fmt::Debug;

#[derive(Debug, Clone, Component)]
#[storage(VecStorage)]
pub struct RenderComponent {
    /// simple color
    color: ColorRgb,

    /// simple 2D shape
    shape: PhysicalShape,
}

impl RenderComponent {
    pub fn new(color: ColorRgb, shape: PhysicalShape) -> Self {
        Self { color, shape }
    }

    pub fn color(&self) -> ColorRgb {
        self.color
    }
    pub fn shape(&self) -> PhysicalShape {
        self.shape
    }
}

/// Wrapper for calling generic Renderer in render system
pub struct RenderSystem<'a, R: Renderer> {
    pub renderer: &'a mut R,
    pub slices: SliceRange,
    pub interpolation: f32,
}

impl<'a, R: Renderer> System<'a> for RenderSystem<'a, R> {
    type SystemData = (
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, RenderComponent>,
        ReadStorage<'a, SelectedComponent>,
    );

    fn run(&mut self, (transform, render, selected): Self::SystemData) {
        for (transform, render, selected) in (&transform, &render, selected.maybe()).join() {
            if self.slices.contains(transform.slice()) {
                // make copy to mutate for interpolation
                let mut transform = *transform;

                transform.position = {
                    let last_pos: Vector3 = transform.last_position.into();
                    let curr_pos: Vector3 = transform.position.into();
                    last_pos.lerp(curr_pos, self.interpolation).into()
                };

                self.renderer.sim_entity(&transform, render);

                if selected.is_some() {
                    self.renderer.sim_selected(&transform);
                }
            }
        }
    }
}
