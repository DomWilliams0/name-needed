use world::WorldViewer;

use crate::input::InputCommand;
use crate::perf::PerfAvg;
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

pub trait InitializedSimulationBackend: Sized {
    type Renderer: Renderer;
    type Persistent: PersistentSimulationBackend<Initialized = Self>;

    fn consume_events(&mut self) -> EventsOutcome;

    fn tick(&mut self);

    fn render(
        &mut self,
        simulation: &mut Simulation<Self::Renderer>,
        interpolation: f64,
        perf: &PerfAvg,
        commands: &mut Vec<InputCommand>,
    );

    fn end(self) -> Self::Persistent;
}

pub trait PersistentSimulationBackend: Sized {
    type Error: Debug; // TODO Error!!
    type Initialized: InitializedSimulationBackend<Persistent = Self>;

    /// One time setup
    fn new() -> Result<Self, Self::Error>;

    fn start(self, world: WorldViewer) -> Self::Initialized;
}

pub mod state {
    use crate::{InitializedSimulationBackend, PersistentSimulationBackend};
    use world::WorldViewer;

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
        pub fn new() -> Result<Self, <B as PersistentSimulationBackend>::Error> {
            let backend = B::new()?;
            Ok(Self(State::Uninit(backend)))
        }

        pub fn start(&mut self, world: WorldViewer) -> &mut B::Initialized {
            let state = std::mem::replace(&mut self.0, State::_Ephemeral);

            self.0 = match state {
                State::Uninit(b) => State::Init(b.start(world)),
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
