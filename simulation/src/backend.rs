use crate::{Renderer, Simulation};
use world::WorldViewer;

pub enum EventsOutcome {
    Continue,
    Exit,
}

pub trait SimulationBackend {
    type Renderer: Renderer;
    // TODO result instead of panicking
    fn new(world_viewer: WorldViewer) -> Self;

    fn consume_events(&mut self) -> EventsOutcome;

    fn tick(&mut self);

    fn render(&mut self, simulation: &mut Simulation<Self::Renderer>, interpolation: f64);
}
