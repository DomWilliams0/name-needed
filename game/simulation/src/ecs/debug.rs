use crate::ecs::name::KindComponent;
use crate::ecs::*;
use crate::render::DebugRenderer;
use crate::{
    InnerWorldRef, ItemStackComponent, Renderer, ThreadedWorldLoader, TransformComponent,
    WorldViewer,
};
use std::fmt::Write;
use unit::world::WorldPoint;

#[derive(Default)]
pub struct EntityIdDebugRenderer(String);

#[derive(Default)]
pub struct EntityNameDebugRenderer(String);

impl<R: Renderer> DebugRenderer<R> for EntityIdDebugRenderer {
    fn identifier(&self) -> &'static str {
        "entity ids"
    }

    fn name(&self) -> &'static str {
        "Entity IDs\0"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        ecs_world: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        type Query<'a> = (Read<'a, EntitiesRes>, ReadStorage<'a, TransformComponent>);

        let (entities, transforms) = <Query as SystemData>::fetch(ecs_world);
        let slices = viewer.entity_range();

        for (entity, transform) in (&entities, &transforms).join() {
            if !slices.contains(transform.slice()) {
                continue;
            }

            self.0.clear();
            let _ = write!(&mut self.0, "{}", Entity::from(entity));

            // out the way of other text
            const OFFSET: f32 = 0.7;

            let mut pos = transform.position;
            pos.modify_y(|y| y + OFFSET);
            renderer.debug_text(pos, &self.0);
        }
    }
}
