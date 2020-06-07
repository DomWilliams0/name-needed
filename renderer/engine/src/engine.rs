use std::time::Duration;

use common::*;
use gameloop::{FrameAction, GameLoop};
use simulation::input::InputCommand;
use simulation::{
    self, EventsOutcome, ExitType, Perf, Renderer, Simulation, SimulationBackend, WorldViewer,
};

pub struct Engine<R: Renderer, B: SimulationBackend<Renderer = R>> {
    backend: B,
    simulation: Simulation<R>,
    perf: Perf,
    /// Commands from UI -> game, accumulated over render frames and passed to sim on each tick
    sim_input_commands: Vec<InputCommand>,
}

impl<R: Renderer, B: SimulationBackend<Renderer = R>> Engine<R, B> {
    pub fn new(simulation: Simulation<R>) -> Result<Self, B::Error> {
        let viewer = WorldViewer::from_world(simulation.world());
        let backend = B::new(viewer)?;

        Ok(Self {
            backend,
            simulation,
            perf: Default::default(),
            sim_input_commands: Vec::with_capacity(32),
        })
    }

    /// Game loop
    pub fn run(mut self) -> ExitType {
        // initial sleep
        let delay = config::get().simulation.start_delay;
        if delay > 0 {
            info!("sleeping for {}ms before starting", delay);
            std::thread::sleep(Duration::from_millis(delay as u64));
        }

        // TODO separate faster rate for physics?
        let game_loop = GameLoop::new(simulation::TICKS_PER_SECOND, 5);

        loop {
            let frame = game_loop.start_frame();

            match self.backend.consume_events() {
                EventsOutcome::Continue => {}
                EventsOutcome::Exit(e) => break e,
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
        let _timer = self.perf.tick.time();

        self.simulation.tick(&self.sim_input_commands);
        self.sim_input_commands.clear();

        self.backend.tick();
    }

    fn render(&mut self, interpolation: f64) {
        let perf = self.perf.calculate();

        trace!("render (interpolation={})", interpolation);
        let _timer = self.perf.render.time();

        self.backend.render(
            &mut self.simulation,
            interpolation,
            &perf,
            &mut self.sim_input_commands,
        );
    }
}
