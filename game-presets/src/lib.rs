use std::path::Path;

pub use dev::DevGamePreset;
pub use empty::EmptyGamePreset;
use simulation::{Renderer, Simulation};
use world::WorldRef;

pub trait GamePreset<R: Renderer> {
    fn name(&self) -> &str;
    fn config(&self) -> Option<&Path> {
        None
    }
    fn world(&self) -> WorldRef {
        WorldRef::new(world::presets::from_config())
    }
    fn init(&self, sim: &mut Simulation<R>);

    fn load(&self) -> Simulation<R> {
        let world = self.world();
        let mut sim = Simulation::new(world);
        self.init(&mut sim);
        sim
    }
}

mod dev;
mod empty;
