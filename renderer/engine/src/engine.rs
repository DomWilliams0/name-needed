use std::time::Duration;

use common::*;
use gameloop::{FrameAction, GameLoop};
use simulation::input::InputCommand;
use simulation::{self, Exit, InitializedSimulationBackend, Perf, Renderer, Simulation};

pub struct Engine<'b, R: Renderer, B: InitializedSimulationBackend<Renderer = R>> {
    backend: &'b mut B,
    simulation: Simulation<R>,
    perf: Perf,
    /// Commands from UI -> game, accumulated over render frames and passed to sim on each tick
    sim_input_commands: Vec<InputCommand>,
}

impl<'b, R: Renderer, B: InitializedSimulationBackend<Renderer = R>> Engine<'b, R, B> {
    pub fn new(simulation: Simulation<R>, backend: &'b mut B) -> Self {
        Self {
            backend,
            simulation,
            perf: Default::default(),
            sim_input_commands: Vec::with_capacity(32),
        }
    }

    /// Game loop
    pub fn run(mut self) -> Exit {
        // initial sleep
        let delay = config::get().simulation.start_delay;
        if delay > 0 {
            info!("sleeping for {}ms before starting", delay);
            std::thread::sleep(Duration::from_millis(delay as u64));
        }

        let game_loop = match GameLoop::new(simulation::TICKS_PER_SECOND, 5) {
            Err(e) => {
                panic!("game loop initialization failed: {}", e);
            }
            Ok(gl) => gl,
        };

        loop {
            if let Some(exit) = self.backend.consume_events() {
                break exit;
            }

            for action in game_loop.actions() {
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

        let world_viewer = self.backend.world_viewer();
        self.simulation.tick(&self.sim_input_commands, world_viewer);
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
