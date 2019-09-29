use pyro::{SoaStorage, World as EcsWorld, World};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TickFrequency {
    Critical,
    Regularly,
    Rarely,
}

impl TickFrequency {
    /// How many X game ticks to tick
    pub fn stride(self) -> usize {
        match self {
            TickFrequency::Critical => 1,
            TickFrequency::Regularly => 4,
            TickFrequency::Rarely => 40,
        }
    }
}

pub struct SystemWrapper {
    pub system: Box<dyn System>,
    pub last_tick: usize,
}

pub trait System {
    fn tick_frequency(&self) -> TickFrequency;

    fn tick(&mut self, world: &EcsWorld, ticks_since_last: usize);
}
