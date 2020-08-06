use crate::ecs::*;
use crate::input::{SelectedComponent, SelectedTiles};
use crate::render::renderer::Renderer;
use crate::render::shape::RenderHexColor;
use crate::{SliceRange, TransformComponent};
use color::ColorRgb;
use common::*;
use specs::{Builder, EntityBuilder};

#[derive(Debug, Clone, Component)]
#[storage(VecStorage)]
pub struct RenderComponent {
    /// simple color
    pub color: ColorRgb,
}

impl RenderComponent {
    pub fn new(color: ColorRgb) -> Self {
        Self { color }
    }

    pub const fn color(&self) -> ColorRgb {
        self.color
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
        Read<'a, SelectedTiles>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, RenderComponent>,
        ReadStorage<'a, SelectedComponent>,
    );

    fn run(&mut self, (selected_block, transform, render, selected): Self::SystemData) {
        for (transform, render, selected) in (&transform, &render, selected.maybe()).join() {
            if self.slices.contains(transform.slice()) {
                // make copy to mutate for interpolation
                let mut transform = transform.clone();

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

        Ok(Box::new(Self {
            color: color.into(),
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

register_component_template!("render", RenderComponent);
