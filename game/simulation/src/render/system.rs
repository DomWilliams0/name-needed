use crate::ecs::*;
use crate::input::{SelectedComponent, SelectedTiles};
use crate::render::renderer::Renderer;
use crate::render::shape::RenderHexColor;
use crate::transform::PhysicalComponent;
use crate::{Shape2d, SliceRange, TransformComponent};
use color::ColorRgb;
use common::*;
use std::convert::TryInto;

#[derive(Debug, Clone, Component, EcsComponent)]
#[storage(VecStorage)]
#[name("render")]
pub struct RenderComponent {
    /// Simple 2d shape
    pub shape: Shape2d,

    /// Simple color
    pub color: ColorRgb,
}

/// Wrapper for calling generic Renderer in render system
pub struct RenderSystem<'a, R: Renderer> {
    pub renderer: &'a mut R,
    pub slices: SliceRange,
    pub interpolation: f32,
}

impl<'a, R: Renderer> System<'a> for RenderSystem<'a, R> {
    type SystemData = (
        Read<'a, SelectedTiles>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, RenderComponent>,
        ReadStorage<'a, PhysicalComponent>,
        ReadStorage<'a, SelectedComponent>,
    );

    fn run(&mut self, (selected_block, transform, render, physical, selected): Self::SystemData) {
        for (transform, render, physical, selected) in
            (&transform, &render, &physical, selected.maybe()).join()
        {
            if self.slices.contains(transform.slice()) {
                // make copy to mutate for interpolation
                let mut transform = transform.clone();

                transform.position = {
                    let last_pos: Vector3 = transform.last_position.into();
                    let curr_pos: Vector3 = transform.position.into();
                    let lerped = last_pos.lerp(curr_pos, self.interpolation);
                    lerped.try_into().expect("invalid lerp")
                };

                self.renderer.sim_entity(&transform, render);

                if selected.is_some() {
                    self.renderer.sim_selected(&transform, &physical);
                }
            }
        }

        if let Some((from, to)) = selected_block.bounds() {
            self.renderer
                .tile_selection(from, to, ColorRgb::new(230, 240, 230));
        }
    }
}

impl<V: Value> ComponentTemplate<V> for RenderComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let color: RenderHexColor = values.get("color")?.into_type()?;
        let shape: Shape2d = values.get("shape")?.into_type()?;

        Ok(Box::new(Self {
            color: color.into(),
            shape,
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

register_component_template!("render", RenderComponent);
