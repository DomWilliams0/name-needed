use env_logger;
use log::{error, info, warn};

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
        warn!(
            "failed to init debug tweaker, falling back on default values ({})",
            e
        );
    }

    let world = {
        info!("creating world");
        // TODO load this from a config
        world::WorldRef::new(world::presets::multi_chunk_wonder())
    };
    let eng = engine::Engine::new(world);
    eng.run();
    info!("cleanly exiting");
}
