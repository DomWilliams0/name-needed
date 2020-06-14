use std::time::{Duration, Instant};

use simulation::input::InputCommand;
use simulation::{
    EventsOutcome, ExitType, InitializedSimulationBackend, PerfAvg, PersistentSimulationBackend,
    RenderComponent, Renderer, Simulation, TransformComponent, WorldViewer,
};

pub struct DummyRenderer;

pub struct DummyBackendPersistent;
pub struct DummyBackendInit {
    end_time: Instant,
}

impl Renderer for DummyRenderer {
    type Target = ();
    type Error = ();

    fn init(&mut self, _target: Self::Target) {}

    fn sim_start(&mut self) {}

    fn sim_entity(&mut self, _transform: &TransformComponent, _render: &RenderComponent) {}

    fn sim_selected(&mut self, _transform: &TransformComponent) {}

    fn sim_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn deinit(&mut self) -> Self::Target {}
}

impl InitializedSimulationBackend for DummyBackendInit {
    type Renderer = DummyRenderer;
    type Persistent = DummyBackendPersistent;

    fn consume_events(&mut self) -> EventsOutcome {
        if Instant::now() > self.end_time {
            EventsOutcome::Exit(ExitType::Stop)
        } else {
            EventsOutcome::Continue
        }
    }

    fn tick(&mut self) {}

    fn render(
        &mut self,
        _: &mut Simulation<Self::Renderer>,
        _: f64,
        _: &PerfAvg,
        _: &mut Vec<InputCommand>,
    ) {
    }

    fn end(self) -> Self::Persistent {
        DummyBackendPersistent
    }
}

impl PersistentSimulationBackend for DummyBackendPersistent {
    type Error = ();
    type Initialized = DummyBackendInit;

    fn new() -> Result<Self, Self::Error> {
        Ok(Self)
    }

    fn start(self, _: WorldViewer) -> Self::Initialized {
        DummyBackendInit {
            end_time: Instant::now() + Duration::from_secs(10),
        }
    }
}
