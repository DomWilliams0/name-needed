pub use dev::DevGamePreset;
use simulation::{Renderer, Simulation};
use std::path::Path;
use world::WorldRef;

pub trait GamePreset {
    fn name(&self) -> &str;
    fn config(&self) -> Option<&Path> {
        None
    }
    fn world(&self) -> WorldRef;
    fn init<R: Renderer>(&self, sim: &mut Simulation<R>);

    fn load<R: Renderer>(&self) -> Simulation<R> {
        let world = self.world();
        let mut sim = Simulation::new(world);
        self.init(&mut sim);
        sim
    }
}

mod dev;
