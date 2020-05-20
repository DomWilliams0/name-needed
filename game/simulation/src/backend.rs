use world::WorldViewer;

use crate::{Renderer, Simulation};
use std::fmt::Debug;

pub enum ExitType {
    Stop,
    Restart,
}

/// Action to take after consuming events
pub enum EventsOutcome {
    Continue,
    Exit(ExitType),
}

pub trait SimulationBackend: Sized {
    type Renderer: Renderer;
    type Error: Debug;
    fn new(world_viewer: WorldViewer) -> Result<Self, Self::Error>;

    fn consume_events(&mut self) -> EventsOutcome;

    fn tick(&mut self);

    fn render(&mut self, simulation: &mut Simulation<Self::Renderer>, interpolation: f64);
}
