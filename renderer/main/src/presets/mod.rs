use common::*;
use std::path::Path;

pub use dev::DevGamePreset;
pub use empty::EmptyGamePreset;
use simulation::{BlockForAllResult, Renderer, Simulation, ThreadedWorldLoader};
use std::time::Duration;

pub trait GamePreset<R: Renderer> {
    fn name(&self) -> &str;
    fn config(&self) -> Option<&Path> {
        None
    }
    fn world(&self) -> ThreadedWorldLoader;
    fn init(&self, sim: &mut Simulation<R>);

    fn load(&self) -> Simulation<R> {
        let mut world = self.world();

        debug!("waiting for world to load before initializing simulation");
        world.request_all_chunks();
        match world.block_for_all(Duration::from_secs(10)) {
            BlockForAllResult::Success => {}
            err => {
                error!("failed to wait for world to load: {:?}", err);
                panic!("failed to wait for world to load: {:?}", err); // TODO return result
            }
        }

        let mut sim = Simulation::new(world);
        self.init(&mut sim);
        sim
    }
}

mod dev;
mod empty;
