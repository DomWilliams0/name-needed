use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use color::ColorRgb;
use common::*;
use debug_draw::DebugDrawer;
use world::{SliceRange, WorldRef};

use crate::ecs::{create_ecs_world, entity_id, EcsWorld, Entity, System, TickData};
use crate::movement::{DesiredMovementComponent, MovementFulfilmentSystem};
use crate::path::{FollowPathComponent, PathDebugRenderer, PathSteeringSystem,
                  TempPathAssignmentSystem};
use crate::physics::{PhysicsComponent, PhysicsSystem};
use crate::render::dummy::DummyDebugRenderer;
use crate::render::{DebugRenderer, FrameRenderState, PhysicalComponent, RenderSystem, Renderer};
use crate::steer::{SteeringComponent, SteeringSystem};
use crate::sync::{SyncFromPhysicsSystem, SyncToPhysicsSystem};
use crate::transform::TransformComponent;

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    renderer: PhantomData<R>,
    debug_renderers: Vec<Box<dyn DebugRenderer<R>>>,
    debug_physics: bool,
}

impl<R: Renderer> Simulation<R> {
    pub fn new(world: WorldRef) -> Self {
        // unconditionally add physics debug renderer
        world
            .borrow_mut()
            .physics_world_mut()
            .set_debug_drawer(true);

        Self {
            ecs_world: create_ecs_world(),
            renderer: PhantomData,
            voxel_world: world,
            debug_renderers: vec![Box::new(DummyDebugRenderer), Box::new(PathDebugRenderer)],
            debug_physics: config::get().display.debug_physics,
        }
    }

    // TODO return result
    // TODO entity builder
    pub fn add_entity(
        &mut self,
        block_pos: (i32, i32, Option<i32>),
        color: ColorRgb,
        dimensions: (f32, f32, f32),
    ) -> &[Entity] {
        let world = &self.voxel_world;
        let transform = match block_pos {
            (x, y, Some(z)) => TransformComponent::from_block_center(x, y, z),
            (x, y, None) => {
                let mut transform =
                    TransformComponent::from_highest_safe_point(&world.borrow(), x, y)
                        .expect("should be valid position");

                // stand on top
                transform.position.2 += dimensions.2 / 4.0;

                transform
            }
        };

        let physical = PhysicalComponent { color, dimensions };
        let physics = PhysicsComponent::new(world.borrow_mut(), &transform, &physical);

        info!("adding an entity at {:?}", transform.position);

        let entities = self.ecs_world.insert(
            (),
            vec![(
                transform,
                DesiredMovementComponent::default(),
                physical,
                physics,
                FollowPathComponent::default(),
                // Steering::seek(WorldPoint(15.0, 3.0, 3.0)),
                SteeringComponent::default(),
            )],
        );

        for &e in entities {
            event_verbose(Event::Entity(EntityEvent::Create(entity_id(e))))
        }
        entities
    }

    fn tick_data(&mut self) -> TickData {
        TickData {
            voxel_world: self.voxel_world.clone(),
            ecs_world: &mut self.ecs_world,
        }
    }

    pub fn tick(&mut self) {
        // tick systems
        let _span = enter_span(Span::Tick);
        let mut tick_data = self.tick_data();

        // assign paths
        TempPathAssignmentSystem.tick_system(&mut tick_data);

        // follow paths with steering
        PathSteeringSystem.tick_system(&mut tick_data);

        // apply steering
        SteeringSystem.tick_system(&mut tick_data);

        // attempt to fulfil desired velocity
        MovementFulfilmentSystem.tick_system(&mut tick_data);

        // apply physics
        SyncToPhysicsSystem.tick_system(&mut tick_data);
        PhysicsSystem.tick_system(&mut tick_data);
        SyncFromPhysicsSystem.tick_system(&mut tick_data);
    }

    pub fn world(&self) -> WorldRef {
        self.voxel_world.clone()
    }

    // target is for this frame only
    pub fn render(
        &mut self,
        slices: SliceRange,
        target: Rc<RefCell<R::Target>>,
        renderer: &mut R,
        interpolation: f64,
    ) {
        let frame_state = FrameRenderState { target, slices };

        // start frame
        renderer.init(frame_state.target.clone());

        // render simulation
        {
            renderer.start();
            {
                let mut render_system = RenderSystem {
                    renderer,
                    frame_state: frame_state.clone(),
                    interpolation,
                };

                render_system.tick_system(&mut self.tick_data());
            }
            renderer.finish();
        }

        // render debug shapes
        // TODO needs interpolation?
        {
            renderer.debug_start();

            for debug_renderer in self.debug_renderers.iter_mut() {
                debug_renderer.render(
                    renderer,
                    self.voxel_world.clone(),
                    &self.ecs_world,
                    &frame_state,
                );
            }
            if self.debug_physics {
                DebugDrawer.render(
                    renderer,
                    self.voxel_world.clone(),
                    &self.ecs_world,
                    &frame_state,
                );
            }

            renderer.debug_finish();
        }

        // end frame
        renderer.deinit();
    }

    /// Toggles and returns new enabled state
    pub fn toggle_physics_debug_rendering(&mut self) -> bool {
        self.debug_physics = !self.debug_physics;
        self.debug_physics
    }
}
