use std::time::Duration;

use crate::panic;
use common::*;
use simulation::input::UiCommand;
use simulation::{self, Exit, InitializedSimulationBackend, Perf, Renderer, Simulation};

pub struct Engine<'b, R: Renderer, B: InitializedSimulationBackend<Renderer = R>> {
    backend: &'b mut B,
    simulation: Simulation<R>,
    perf: Perf,
    /// Commands from UI -> game, accumulated over render frames and passed to sim on each tick
    sim_ui_commands: Vec<UiCommand>,
}

impl<'b, R: Renderer, B: InitializedSimulationBackend<Renderer = R>> Engine<'b, R, B> {
    pub fn new(simulation: Simulation<R>, backend: &'b mut B) -> Self {
        Self {
            backend,
            simulation,
            perf: Default::default(),
            sim_ui_commands: Vec::with_capacity(32),
        }
    }

    /// Game loop
    pub fn run(mut self) -> Exit {
        // initial sleep
        let delay = config::get().simulation.start_delay;
        if delay > 0 {
            info!("sleeping for {delay}ms before starting", delay = delay);
            std::thread::sleep(Duration::from_millis(delay as u64));
        }

        #[cfg(not(feature = "lite"))]
        let game_loop = gameloop::GameLoop::new(simulation::TICKS_PER_SECOND, 5)
            .expect("game loop initialization failed");

        loop {
            if panic::has_panicked() {
                debug!("breaking out of loop due to panics");
                break Exit::Stop;
            }

            if let Some(exit) = self.backend.consume_events() {
                break exit;
            }

            #[cfg(not(feature = "lite"))]
            for action in game_loop.actions() {
                match action {
                    gameloop::FrameAction::Tick => self.tick(),
                    gameloop::FrameAction::Render { interpolation } => self.render(interpolation),
                }
            }

            #[cfg(feature = "lite")]
            {
                // tick as fast as possible
                self.tick();
            }
        }
    }

    fn tick(&mut self) {
        trace!("tick");
        let _timer = self.perf.tick.time();

        let world_viewer = self.backend.world_viewer();
        self.simulation
            .tick(self.sim_ui_commands.drain(..), world_viewer);

        self.backend.tick();
    }

    fn render(&mut self, interpolation: f64) {
        let perf = self.perf.calculate();

        trace!("render"; "interpolation" => interpolation);
        let _timer = self.perf.render.time();

        self.backend.render(
            &mut self.simulation,
            interpolation,
            &perf,
            &mut self.sim_ui_commands,
        );
    }
}
