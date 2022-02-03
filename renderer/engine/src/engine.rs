use std::time::Duration;

use gameloop::GameLoop;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use common::*;
use simulation::input::UiCommands;
use simulation::{
    self, BackendData, Exit, GameSpeedChange, InitializedSimulationBackend, Perf, Renderer,
    Simulation, TickResponse,
};

#[derive(Copy, Clone, FromPrimitive, Debug)]
#[num_traits = "num_traits"]
#[repr(u8)]
enum RunSpeed {
    Slowest,
    Slower,
    Normal,
    Faster,
    Fastest,
}

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
        use simulation::ComponentWorld;
        let runtime = self.simulation.world().resource::<simulation::Runtime>();
        testing::HookContext {
            simulation: self.simulation.as_lite_ref(),
            commands: &mut self.sim_ui_commands,
            events: runtime.event_log(),
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

        let mut speed = RunSpeed::Normal;
        let mut game_loop = RunSpeed::Normal.into_gameloop();

        self.backend.start(&mut self.sim_ui_commands);

        let mut tick = TickResponse::default();
        loop {
            if panik::has_panicked() {
                debug!("breaking out of loop due to panics");
                break Exit::Stop;
            }

            let backend_data = self.backend.consume_events(&mut self.sim_ui_commands);

            #[cfg(not(feature = "lite"))]
            for action in game_loop.actions() {
                match action {
                    gameloop::FrameAction::Tick => {
                        self.tick(&backend_data, &mut tick);
                    }
                    gameloop::FrameAction::Render { interpolation } => self.render(interpolation),
                }
            }

            #[cfg(feature = "lite")]
            {
                // tick as fast as possible
                self.tick(&backend_data, &mut tick);
            }

            let tick = std::mem::take(&mut tick);
            if let Some(exit) = tick.exit {
                info!("exiting game"; "reason" => ?exit);
                break exit;
            }

            if let Some(change) = tick.speed_change {
                if let Some(new_speed) = speed.try_change(change) {
                    info!("new game speed"; "speed" => ?new_speed);
                    speed = new_speed;
                    game_loop = new_speed.into_gameloop();
                }
            }
        }
    }

    fn tick(&mut self, backend_data: &BackendData, response: &mut TickResponse) {
        trace!("tick");
        {
            let _timer = self.perf.tick.time();

            let world_viewer = self.backend.world_viewer();

            let commands = self.sim_ui_commands.drain(..);
            self.simulation
                .tick(commands, world_viewer, backend_data, response)
        }

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
                        response.exit = Some(Exit::TestSuccess);
                    }
                    HookResult::TestFailure(err) => {
                        error!("test failed: {}", err);
                        response.exit = Some(Exit::TestFailure(err));
                    }
                }
            }
        }
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

impl RunSpeed {
    fn into_gameloop(self) -> GameLoop {
        let mul = match self {
            RunSpeed::Slowest => 0.2,
            RunSpeed::Slower => 0.5,
            RunSpeed::Normal => 1.0,
            RunSpeed::Faster => 2.5,
            RunSpeed::Fastest => 5.0,
        };

        let tps = ((simulation::TICKS_PER_SECOND as f32) * mul) as usize;
        GameLoop::new(tps, 5).expect("bad gameloop parameters")
    }

    fn try_change(self, change: GameSpeedChange) -> Option<Self> {
        let delta = match change {
            GameSpeedChange::Faster => 1,
            GameSpeedChange::Slower => -1,
        };

        Self::from_i8((self as i8) + delta)
    }
}
