use std::time::{Duration, Instant};

use color::ColorRgb;
use simulation::{
    EventsOutcome, ExitType, PhysicalComponent, Renderer, Simulation, SimulationBackend,
    TransformComponent, WorldViewer,
};

pub struct DummyRenderer;

pub struct DummyBackend {
    end_time: Instant,
}

impl Renderer for DummyRenderer {
    type Target = ();

    fn init(&mut self, _target: Self::Target) {}

    fn sim_start(&mut self) {}

    fn sim_entity(&mut self, _transform: &TransformComponent, _physical: &PhysicalComponent) {}

    fn sim_finish(&mut self) {}

    fn deinit(&mut self) -> Self::Target {}
}

impl SimulationBackend for DummyBackend {
    type Renderer = DummyRenderer;

    fn new(_world_viewer: WorldViewer) -> Self {
        Self {
            end_time: Instant::now() + Duration::from_secs(5),
        }
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
