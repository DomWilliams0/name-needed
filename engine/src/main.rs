use tweaker;
use world;

mod engine;

fn main() {
    if let Err(e) = tweaker::init() {
        eprintln!("[engine] failed to init debug tweaker: {}", e);
        std::process::exit(1);
    }

    let mut world = world::World::default();
    let eng = engine::Engine::new(&mut world);
    eng.run();
}
