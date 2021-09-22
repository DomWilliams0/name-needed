use std::time::Duration;

use common::*;
use simulation::input::UiCommands;
use simulation::{self, Exit, InitializedSimulationBackend, Perf, Renderer, Simulation};

pub struct Engine<'b, R: Renderer, B: InitializedSimulationBackend<Renderer = R>> {
    backend: &'b mut B,
    simulation: Simulation<R>,
    perf: Perf,
    /// Commands from UI -> game, accumulated over render frames and passed to sim on each tick
    sim_ui_commands: UiCommands,
    #[cfg(feature = "hook")]
    tick_hook: Option<testing::TickHookThunk>,
}

impl<'b, R: Renderer, B: InitializedSimulationBackend<Renderer = R>> Engine<'b, R, B> {
    pub fn new(simulation: Simulation<R>, backend: &'b mut B) -> Self {
        Self {
            backend,
            simulation,
            perf: Default::default(),
            sim_ui_commands: Vec::with_capacity(32),
            #[cfg(feature = "hook")]
            tick_hook: None,
        }
    }

    #[cfg(feature = "hook")]
    pub fn set_tick_hook(&mut self, hook: Option<testing::TickHookThunk>) {
        self.tick_hook = hook
    }

    #[cfg(feature = "hook")]
    pub fn hook_context(&mut self) -> testing::HookContext {
        testing::HookContext {
            simulation: self.simulation.as_lite_ref(),
            commands: &mut self.sim_ui_commands,
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

        let mut exit = None;

        self.backend.start(&mut self.sim_ui_commands);

        loop {
            if panik::has_panicked() {
                debug!("breaking out of loop due to panics");
                break Exit::Stop;
            }

            self.backend.consume_events(&mut self.sim_ui_commands);

            #[cfg(not(feature = "lite"))]
            for action in game_loop.actions() {
                match action {
                    gameloop::FrameAction::Tick => {
                        exit = self.tick();
                    }
                    gameloop::FrameAction::Render { interpolation } => self.render(interpolation),
                }
            }

            #[cfg(feature = "lite")]
            {
                // tick as fast as possible
                exit = self.tick();
            }

            if let Some(exit) = exit {
                info!("exiting game"; "reason" => ?exit);
                break exit;
            }
        }
    }

    fn tick(&mut self) -> Option<Exit> {
        trace!("tick");
        let exit = {
            let _timer = self.perf.tick.time();

            let world_viewer = self.backend.world_viewer();

            let commands = self.sim_ui_commands.drain(..);
            self.simulation.tick(commands, world_viewer)
        };

        self.backend.tick();

        #[cfg(feature = "hook")]
        {
            use testing::HookResult;
            if let Some(hook) = self.tick_hook {
                let ctx = self.hook_context();
                match hook(&ctx) {
                    HookResult::KeepGoing => {}
                    HookResult::TestSuccess => {
                        info!("test finished successfully");
                        testing::destroy_hook();
                        return Some(Exit::Stop);
                    }
                    HookResult::TestFailure(err) => {
                        error!("test failed: {}", err);
                        testing::destroy_hook();
                        return Some(Exit::Abort(err));
                    }
                }
            }
        }

        exit
    }

    fn render(&mut self, interpolation: f64) {
        let perf = self.perf.calculate();

        trace!("render"; "interpolation" => interpolation);
        let _timer = self.perf.render.time();

        self.backend.render(
            &mut self.simulation,
            interpolation,
            perf,
            &mut self.sim_ui_commands,
        );
    }
}
