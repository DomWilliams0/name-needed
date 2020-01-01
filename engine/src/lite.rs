use std::time::{Duration, Instant};

use simulation::{EventsOutcome, Physical, Renderer, Simulation, SimulationBackend, Transform};
use unit::view::ViewPoint;
use world::WorldViewer;

pub struct DummyRenderer;

pub struct DummyBackend {
    end_time: Instant,
}

impl Renderer for DummyRenderer {
    type Target = ();

    fn entity(&mut self, transform: &Transform, physical: &Physical) {}

    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb) {}

    fn debug_add_tri(&mut self, points: [ViewPoint; 3], color: ColorRgb) {}
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
            EventsOutcome::Exit
        } else {
            EventsOutcome::Continue
        }
    }

    fn tick(&mut self) {}

    fn render(&mut self, simulation: &mut Simulation<Self::Renderer>, interpolation: f64) {}
}
