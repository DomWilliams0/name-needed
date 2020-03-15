use std::time::{Duration, Instant};

use color::ColorRgb;
use simulation::{
    EventsOutcome, ExitType, PhysicalComponent, Renderer, Simulation, SimulationBackend,
    TransformComponent, WorldViewer,
};
use unit::view::ViewPoint;

pub struct DummyRenderer;

pub struct DummyBackend {
    end_time: Instant,
}

impl Renderer for DummyRenderer {
    type Target = ();

    fn entity(&mut self, _transform: &TransformComponent, physical: &PhysicalComponent) {}

    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb) {}

    fn debug_add_tri(&mut self, _points: [ViewPoint; 3], color: ColorRgb) {}
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
