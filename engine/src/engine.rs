use common::*;
use gameloop::{FrameAction, GameLoop};
use simulation::{self, EventsOutcome, Renderer, Simulation, SimulationBackend};
use world::WorldViewer;

pub struct Engine<R: Renderer, B: SimulationBackend<Renderer = R>> {
    backend: B,
    simulation: Simulation<R>,
}

impl<R: Renderer, B: SimulationBackend<Renderer = R>> Engine<R, B> {
    pub fn new(simulation: Simulation<R>) -> Self {
        let viewer = WorldViewer::from_world(simulation.world());
        let backend = B::new(viewer);

        Self {
            backend,
            simulation,
        }
    }

    /// Game loop
    pub fn run(mut self) {
        // TODO separate faster rate for physics?
        let game_loop = GameLoop::new(simulation::TICKS_PER_SECOND, 5);

        loop {
            let frame = game_loop.start_frame();

            match self.backend.consume_events() {
                EventsOutcome::Continue => {}
                EventsOutcome::Exit => break,
            }

            for action in frame.actions() {
                match action {
                    FrameAction::Tick => self.tick(),
                    FrameAction::Render { interpolation } => self.render(interpolation),
                }
            }
        }
    }

    fn tick(&mut self) {
        trace!("tick");
        self.simulation.tick();
        self.backend.tick();
    }

    fn render(&mut self, interpolation: f64) {
        trace!("render (interpolation={})", interpolation);
        self.backend.render(&mut self.simulation, interpolation);
    }
}
