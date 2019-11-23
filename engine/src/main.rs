use env_logger;
use log::*;

use config;
use world;

mod camera;
mod engine;
mod render;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("NN_LOG", "info"))
        .target(env_logger::Target::Stdout)
        .init();

    let config_path = "./config.ron";
    if let Err(e) = config::init(config_path) {
        error!("failed to load config initially: {}", e);
        std::process::exit(1);
    }
    debug!("successfully loaded config from '{}'", config_path);

    let world = {
        info!("creating world");
        world::WorldRef::new(world::presets::from_config())
    };
    let eng = engine::Engine::new(world);
    eng.run();
    info!("cleanly exiting");
}
