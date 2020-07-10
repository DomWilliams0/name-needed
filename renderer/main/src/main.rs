use clap::{App, Arg};
use common::*;
use presets::{DevGamePreset, EmptyGamePreset, GamePreset};
use simulation::state::BackendState;
use simulation::{Exit, InitializedSimulationBackend, PersistentSimulationBackend, WorldViewer};

use engine::Engine;
use std::path::PathBuf;

#[cfg(feature = "count-allocs")]
mod count_allocs {
    use alloc_counter::{count_alloc, AllocCounter};

    type Allocator = std::alloc::System;

    const ALLOCATOR: Allocator = std::alloc::System;

    #[global_allocator]
    static A: AllocCounter<Allocator> = AllocCounter(ALLOCATOR);
}

mod presets;

#[cfg(feature = "use-sdl")]
type Backend = engine::SdlBackendPersistent;

#[cfg(feature = "lite")]
type Backend = engine::DummyBackendPersistent;

type BackendInit = <Backend as PersistentSimulationBackend>::Initialized;
type Renderer = <BackendInit as InitializedSimulationBackend>::Renderer;

fn do_main() -> i32 {
    let args = App::new(env!("CARGO_PKG_NAME"))
        .arg(
            Arg::with_name("preset")
                .short("p")
                .long("preset")
                .help("Game preset")
                .takes_value(true)
                .possible_values(&["dev", "empty"]),
        )
        .arg(
            Arg::with_name("dir")
                .short("d")
                .long("dir")
                .help("Directory to look for game files in")
                .default_value("."),
        )
        .get_matches();

    let preset: Box<dyn GamePreset<Renderer>> = match args.value_of("preset") {
        None | Some("dev") => Box::new(DevGamePreset::<Renderer>::default()),
        Some("empty") => Box::new(EmptyGamePreset::default()),
        _ => unreachable!(),
    };

    let root = args.value_of("dir").unwrap();

    // init logger
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("NN_LOG", "info"))
        .target(env_logger::Target::Stdout)
        .filter_module("hyper", LevelFilter::Info) // keep it down will you
        .filter_module("tokio_reactor", LevelFilter::Info)
        .filter_module("tokio_threadpool", LevelFilter::Info)
        .filter_module("mio", LevelFilter::Info)
        .init();

    info!("using game preset '{}'", preset.name());

    // enable structured logging
    struclog::init();

    // start metrics server
    #[cfg(feature = "metrics")]
    metrics::start_serving();

    // load config
    if let Some(config_file_name) = preset.config() {
        let config_path = {
            let mut path = PathBuf::new();
            path.push(root);
            path.push(config_file_name);
            path
        };

        info!("loading config from '{:?}'", config_path);
        if let Err(e) = config::init(config_path) {
            error!("failed to load config: {}", e);
            return 1;
        }
    }

    // initialize persistent backend
    let mut backend_state = match BackendState::<Backend>::new() {
        Err(e) => {
            error!("failed to initialize engine: {:?}", e);
            return 2;
        }
        Ok(b) => b,
    };

    loop {
        // create simulation
        let simulation = {
            let _span = Span::Setup.begin();
            preset.load()
        };

        // initialize backend with simulation world
        let world_viewer = WorldViewer::from_world(simulation.world());
        let backend = backend_state.start(world_viewer);

        // create and run engine
        let engine = Engine::new(simulation, backend);
        let exit = engine.run();

        // uninitialize backend
        backend_state.end();

        if let Exit::Stop = exit {
            break;
        }
    }

    0
}

fn main() {
    #[cfg(feature = "count-allocs")]
    let exit = {
        use alloc_counter::count_alloc;
        let (counts, result) = count_alloc(|| do_main());
        // TODO more granular - n for engine setup, n for sim setup, n for each frame?
        info!(
            "{} allocations, {} reallocs, {} frees",
            counts.0, counts.1, counts.2
        );
        result
    };

    #[cfg(not(feature = "count-allocs"))]
    let exit = do_main();

    info!("exiting cleanly with exit code {}", exit);
    std::process::exit(exit);
}
