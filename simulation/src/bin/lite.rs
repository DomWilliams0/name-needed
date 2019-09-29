use simulation::{Physical, Position, Renderer, Simulation};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use world::{SliceRange, WorldPoint};

struct DebugRenderer;

impl Renderer for DebugRenderer {
    type Target = (); // unused

    fn entity(&mut self, pos: &Position, physical: &Physical) {
        println!("pos({}, {}, {}), {:?}", pos.x, pos.y, pos.z, physical);
    }

    fn debug_add_line(&mut self, _from: WorldPoint, _to: WorldPoint, _color: (u8, u8, u8)) {}

    fn debug_add_tri(&mut self, _points: [WorldPoint; 3], _color: (u8, u8, u8)) {}
}

fn main() {
    let w = Rc::new(RefCell::new(world::World::default()));
    let mut sim = Simulation::new(w);
    let mut renderer = DebugRenderer;

    let nop = Rc::new(RefCell::new(()));

    loop {
        println!("--- tick ---");

        sim.tick();
        sim.render(SliceRange::null(), nop.clone(), &mut renderer, 0.0);

        std::thread::sleep(Duration::from_millis(50));
    }
}
