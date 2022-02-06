use crate::input::UiCommands;
use crate::perf::PerfAvg;
use crate::{Renderer, Simulation, WorldViewer};
use common::Error;
use resources::Resources;
use unit::world::{WorldPoint2d, WorldPosition};

#[derive(Debug)]
pub enum Exit {
    /// Player requested
    Stop,
    Restart,

    /// Test succeeded
    #[cfg(feature = "testing")]
    TestSuccess,
    /// Test failed
    #[cfg(feature = "testing")]
    TestFailure(String),
}

/// Populated by backend events
#[derive(Default)]
pub struct BackendData {
    /// Mouse position in world space
    pub mouse_position: Option<WorldPoint2d>,
}

#[derive(Copy, Clone)]
pub enum GameSpeedChange {
    Faster,
    Slower,
}

#[derive(Default)]
pub struct TickResponse {
    pub exit: Option<Exit>,
    pub speed_change: Option<GameSpeedChange>,
}

pub trait InitializedSimulationBackend: Sized {
    type Renderer: Renderer;
    type Persistent: PersistentSimulationBackend<Initialized = Self>;

    /// Called once per game (re)start. Outputs commands before the game properly starts
    fn start(&mut self, commands_out: &mut UiCommands);

    fn consume_events(&mut self, commands: &mut UiCommands) -> BackendData;

    fn tick(&mut self);

    fn render(
        &mut self,
        simulation: &mut Simulation<Self::Renderer>,
        interpolation: f64,
        perf: PerfAvg,
        commands: &mut UiCommands,
    );

    fn world_viewer(&mut self) -> &mut WorldViewer;

    fn end(self) -> Self::Persistent;
}

pub trait PersistentSimulationBackend: Sized {
    type Error: Error;
    type Initialized: InitializedSimulationBackend<Persistent = Self>;

    /// One time setup
    fn new(resources: &Resources) -> Result<Self, Self::Error>;

    fn start(self, world: WorldViewer, initial_block: WorldPosition) -> Self::Initialized;

    fn name() -> &'static str;
}

pub mod state {
    use crate::{InitializedSimulationBackend, PersistentSimulationBackend, WorldViewer};
    use resources::Resources;
    use unit::world::WorldPosition;

    #[allow(clippy::manual_non_exhaustive)]
    enum State<B: PersistentSimulationBackend> {
        /// Temporary value to use in place of uninitialized memory, for safe handling of panics
        #[doc(hidden)]
        _Ephemeral,

        Uninit(B),
        Init(B::Initialized),
    }

    pub struct BackendState<B: PersistentSimulationBackend>(State<B>);

    impl<B: PersistentSimulationBackend> BackendState<B> {
        pub fn new(
            resources: &Resources,
        ) -> Result<Self, <B as PersistentSimulationBackend>::Error> {
            let backend = B::new(resources)?;
            Ok(Self(State::Uninit(backend)))
        }

        pub fn start(
            &mut self,
            world: WorldViewer,
            initial_block: WorldPosition,
        ) -> &mut B::Initialized {
            let state = std::mem::replace(&mut self.0, State::_Ephemeral);

            self.0 = match state {
                State::Uninit(b) => State::Init(b.start(world, initial_block)),
                _ => panic!("must be uninitialized to use start()"),
            };

            match &mut self.0 {
                State::Init(b) => b,
                _ => unreachable!(),
            }
        }

        pub fn end(&mut self) {
            let state = std::mem::replace(&mut self.0, State::_Ephemeral);

            self.0 = match state {
                State::Init(b) => State::Uninit(b.end()),
                _ => panic!("must be initialized to use end()"),
            };
        }
    }
}
