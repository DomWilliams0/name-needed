use std::time::{Duration, Instant};

use simulation::{
    EventsOutcome, ExitType, RenderComponent, Renderer, Simulation, SimulationBackend,
    TransformComponent, WorldViewer,
};

pub struct DummyRenderer;

pub struct DummyBackend {
    end_time: Instant,
}

impl Renderer for DummyRenderer {
    type Target = ();
    type Error = ();

    fn init(&mut self, _target: Self::Target) {}

    fn sim_start(&mut self) {}

    fn sim_entity(&mut self, _transform: &TransformComponent, _render: &RenderComponent) {}

    fn sim_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn deinit(&mut self) -> Self::Target {}
}

impl SimulationBackend for DummyBackend {
    type Renderer = DummyRenderer;
    type Error = ();

    fn new(_world_viewer: WorldViewer) -> Result<Self, Self::Error> {
        Ok(Self {
            end_time: Instant::now() + Duration::from_secs(5),
        })
    }

    fn consume_events(&mut self) -> EventsOutcome {
        if Instant::now() > self.end_time {
            EventsOutcome::Exit(ExitType::Stop)
        } else {
            EventsOutcome::Continue
        }
    }

    fn tick(&mut self) {}

    fn render(&mut self, _simulation: &mut Simulation<Self::Renderer>, _interpolation: f64) {}
}
