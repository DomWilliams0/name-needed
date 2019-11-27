use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use log::*;

use simulation::{Physical, Renderer, Simulation, Transform};
use world::{self, SliceRange, ViewPoint, WorldRef};

struct DebugRenderer;

impl Renderer for DebugRenderer {
    type Target = (); // unused

    fn entity(&mut self, _transform: &Transform, _physical: &Physical) {}

    fn debug_add_line(&mut self, _from: ViewPoint, _to: ViewPoint, _color: (u8, u8, u8)) {}

    fn debug_add_tri(&mut self, _points: [ViewPoint; 3], _color: (u8, u8, u8)) {}
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("NN_LOG", "debug"))
        .target(env_logger::Target::Stdout)
        .init();

    config::init("config.ron").expect("config failed to load");

    let w = WorldRef::new(world::presets::one_chunk_wonder());
    let mut sim = Simulation::new(w);
    let mut renderer = DebugRenderer;

    let nop = Rc::new(RefCell::new(()));

    for _ in 0..50 {
        info!("tick");
        sim.tick();
        sim.render(SliceRange::all(), nop.clone(), &mut renderer, 0.0);

        std::thread::sleep(Duration::from_millis(50));
    }

    info!("exiting cleanly");
}
