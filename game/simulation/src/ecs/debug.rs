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

const PAD: f32 = 0.2;

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

            let pos = transform.position - WorldPoint::new_unchecked(PAD, PAD, 0.0);
            renderer.debug_text(pos, &self.0);
        }
    }
}

impl<R: Renderer> DebugRenderer<R> for EntityNameDebugRenderer {
    fn identifier(&self) -> &'static str {
        "entity names"
    }

    fn name(&self) -> &'static str {
        "Entity names\0"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        ecs_world: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        type Query<'a> = (
            ReadStorage<'a, TransformComponent>,
            ReadStorage<'a, KindComponent>,
            ReadStorage<'a, ItemStackComponent>,
        );

        let (transforms, names, stacks) = <Query as SystemData>::fetch(ecs_world);
        let slices = viewer.entity_range();

        for (transform, name, stack_opt) in (&transforms, &names, stacks.maybe()).join() {
            if !slices.contains(transform.slice()) {
                continue;
            }

            self.0.clear();

            let _ = write!(&mut self.0, "{}", name);
            // if let (LabelComponent::StackOf(_), Some(stack)) = (name, stack_opt) {
            //     let _ = write!(&mut self.0, " x{}", stack.stack.total_count());
            // }

            let pos = transform.position - WorldPoint::new_unchecked(PAD, PAD, 0.0);
            renderer.debug_text(pos, &self.0);
        }
    }
}
