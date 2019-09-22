use simulation::{Physical, Position, Renderer, Simulation};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use world::SliceRange;

struct DebugRenderer;

impl Renderer for DebugRenderer {
    type Target = (); // unused

    fn entity(&mut self, pos: &Position, physical: &Physical) {
        println!("pos({}, {}, {}), {:?}", pos.x, pos.y, pos.z, physical);
    }
}

fn main() {
    let mut sim = Simulation::new();
    let mut renderer = DebugRenderer;

    let nop = Rc::new(RefCell::new(()));

    loop {
        println!("--- tick ---");

        sim.tick();
        sim.render(SliceRange::null(), nop.clone(), &mut renderer, 0.0);

        std::thread::sleep(Duration::from_millis(50));
    }
}
