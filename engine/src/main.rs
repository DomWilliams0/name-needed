use env_logger;
use log::{error, info};
use tweaker;
use world;

mod camera;
mod engine;
mod render;

fn on_tweaker_fail(e: tweaker::Error) {
    error!("tweaker failed ({}), exiting", e);
    std::process::exit(1);
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("NN_LOG", "info"))
        .target(env_logger::Target::Stdout)
        .init();

    if let Err(e) = tweaker::init(on_tweaker_fail) {
        error!("failed to init debug tweaker: {}", e);
        std::process::exit(1);
    }

    let world = {
        info!("creating default world");
        world::world_ref(world::World::default())
    };
    let eng = engine::Engine::new(world);
    eng.run();
    info!("cleanly exiting");
}
