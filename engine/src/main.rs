use tweaker;
use world;

mod camera;
mod engine;
mod render;

fn on_tweaker_fail(e: tweaker::Error) {
    println!("[engine] tweaker failed ({}), exiting", e);
    std::process::exit(1);
}

fn main() {
    if let Err(e) = tweaker::init(on_tweaker_fail) {
        eprintln!("[engine] failed to init debug tweaker: {}", e);
        std::process::exit(1);
    }

    let mut world = world::World::default();
    let eng = engine::Engine::new(&mut world);
    eng.run();
}
