use common::*;
use engine::Engine;
use game_presets::{DevGamePreset, GamePreset};

fn main() {
    // TODO choose from env or arg or something
    let preset = DevGamePreset;

    // init logger
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("NN_LOG", "info"))
        .target(env_logger::Target::Stdout)
        .init();

    info!("using game preset '{}'", preset.name());

    // load config
    if let Some(config_path) = preset.config() {
        info!("loading config from '{:?}'", config_path);
        if let Err(e) = config::init(config_path) {
            error!("failed to load config initially: {}", e);
            std::process::exit(1);
        }
    }

    // and away we go
    let sim = preset.load();
    let engine = Engine::new(sim);
    engine.run();
}
