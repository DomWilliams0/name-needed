use common::*;
use std::path::Path;

use resources::resource::Resources;
use simulation::{Renderer, Simulation, ThreadedWorldLoader};
use std::time::Duration;

pub trait GamePreset<R: Renderer> {
    fn name(&self) -> &str;
    fn config(&self) -> Option<&Path> {
        None
    }
    fn world(&self) -> BoxedResult<ThreadedWorldLoader>;
    fn init(&self, sim: &mut Simulation<R>) -> BoxedResult<()>;

    fn load(&self, resources: Resources) -> BoxedResult<Simulation<R>> {
        let mut world = self.world()?;

        debug!("waiting for world to load before initializing simulation");
        world.request_all_chunks();
        world.block_for_all(Duration::from_secs(30))?;

        let mut sim = Simulation::new(world, resources)?;
        self.init(&mut sim)?;
        Ok(sim)
    }
}

mod ci;
mod dev;
mod empty;

pub use ci::ContinuousIntegrationGamePreset;
pub use dev::DevGamePreset;
pub use empty::EmptyGamePreset;
