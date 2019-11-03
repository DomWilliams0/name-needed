use simulation::{Physical, Position, Renderer, Simulation};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use world::{SliceRange, WorldPoint, WorldRef};

struct DebugRenderer;

impl Renderer for DebugRenderer {
    type Target = (); // unused

    fn entity(&mut self, _pos: &Position, _physical: &Physical) {}

    fn debug_add_line(&mut self, _from: WorldPoint, _to: WorldPoint, _color: (u8, u8, u8)) {}

    fn debug_add_tri(&mut self, _points: [WorldPoint; 3], _color: (u8, u8, u8)) {}
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("NN_LOG", "debug"))
        .target(env_logger::Target::Stdout)
        .init();

    let w = WorldRef::new(world::World::default());
    let mut sim = Simulation::new(w);
    let mut renderer = DebugRenderer;

    let nop = Rc::new(RefCell::new(()));

    loop {
        sim.tick();
        sim.render(SliceRange::all(), nop.clone(), &mut renderer, 0.0);

        std::thread::sleep(Duration::from_millis(50));
    }
}
