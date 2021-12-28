use std::time::{Duration, Instant};

use common::*;
use resources::Resources;
use simulation::input::{UiCommand, UiCommands, UiRequest};
use simulation::{
    Exit, InitializedSimulationBackend, PerfAvg, PersistentSimulationBackend, PhysicalComponent,
    RenderComponent, Renderer, Simulation, TransformRenderDescription, UiElementComponent,
    WorldViewer,
};
use unit::world::{WorldPoint, WorldPosition};

pub struct DummyRenderer;

#[derive(Debug, Error)]
#[error("Big dummy")]
pub struct DummyError;

pub struct DummyBackendPersistent;
pub struct DummyBackendInit {
    end_time: Instant,
    world_viewer: WorldViewer,
}

impl Renderer for DummyRenderer {
    type FrameContext = ();
    type Error = DummyError;

    fn init(&mut self, _target: Self::FrameContext) {}

    fn sim_start(&mut self) {}

    fn sim_entity(
        &mut self,
        _transform: &TransformRenderDescription,
        _render: &RenderComponent,
        _physical: &PhysicalComponent,
    ) {
    }

    fn sim_selected(
        &mut self,
        _transform: &TransformRenderDescription,
        _physical: &PhysicalComponent,
    ) {
    }

    fn sim_ui_element(
        &mut self,
        _transform: &TransformRenderDescription,
        _ui: &UiElementComponent,
        _selected: bool,
    ) {
    }

    fn sim_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn debug_text(&mut self, _centre: WorldPoint, _text: &str) {}

    fn deinit(&mut self) -> Self::FrameContext {}
}

impl InitializedSimulationBackend for DummyBackendInit {
    type Renderer = DummyRenderer;
    type Persistent = DummyBackendPersistent;

    fn start(&mut self, _commands_out: &mut UiCommands) {}

    fn consume_events(&mut self, commands: &mut UiCommands) {
        if Instant::now() > self.end_time {
            commands.push(UiCommand::new(UiRequest::ExitGame(Exit::Stop)));
        }
    }

    fn tick(&mut self) {}

    fn render(
        &mut self,
        _: &mut Simulation<Self::Renderer>,
        _: f64,
        _: PerfAvg,
        _: &mut UiCommands,
    ) {
    }

    fn world_viewer(&mut self) -> &mut WorldViewer {
        &mut self.world_viewer
    }

    fn end(self) -> Self::Persistent {
        DummyBackendPersistent
    }
}

impl PersistentSimulationBackend for DummyBackendPersistent {
    type Error = DummyError;
    type Initialized = DummyBackendInit;

    fn new(_: &Resources) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    fn start(self, world_viewer: WorldViewer, _: WorldPosition) -> Self::Initialized {
        DummyBackendInit {
            end_time: Instant::now() + Duration::from_secs(30),
            world_viewer,
        }
    }

    fn name() -> &'static str {
        "Dummy"
    }
}
