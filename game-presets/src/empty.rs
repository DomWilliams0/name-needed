use crate::GamePreset;
use simulation::{Renderer, Simulation};
use world::WorldRef;
use world::presets::one_block_wonder;
use std::path::Path;

#[derive(Default)]
pub struct EmptyGamePreset;

impl<R: Renderer> GamePreset<R> for EmptyGamePreset {
    fn name(&self) -> &str {
        "empty"
    }

    fn config(&self) -> Option<&Path> {
        Some(Path::new("config.ron"))
    }

    fn world(&self) -> WorldRef {
        WorldRef::new(one_block_wonder())
    }

    fn init(&self, _sim: &mut Simulation<R>) {
        // nop
    }
}
