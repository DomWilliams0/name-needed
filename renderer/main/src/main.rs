use clap::{App, Arg};
use common::*;
use presets::{DevGamePreset, EmptyGamePreset, GamePreset};
use simulation::state::BackendState;
use simulation::{
    self, current_tick, Exit, InitializedSimulationBackend, PersistentSimulationBackend,
    WorldViewer,
};

use engine::Engine;
use resources::resource::Resources;
use resources::ResourceContainer;
use std::io::Write;
use std::path::Path;

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

fn do_main() -> BoxedResult<()> {
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

    let root: &Path = args
        .value_of("dir")
        .unwrap() // default is provided
        .as_ref();

    my_info!("chosen game preset"; "preset" => ?preset.name());

    // start metrics server
    #[cfg(feature = "metrics")]
    metrics::start_serving();

    // init resources root
    let resources = Resources::new(root)?;

    // load config
    if let Some(config_file_name) = preset.config() {
        let config_path = resources.get_file(config_file_name)?;

        my_info!("loading config"; "path" => ?config_path);
        config::init(config_path)?;
    }

    // initialize persistent backend
    let mut backend_state = {
        log_scope!(o!("backend" => Backend::name()));
        BackendState::<Backend>::new(&resources)?
    };

    loop {
        // create simulation
        let simulation = preset.load(resources.clone())?;

        // initialize backend with simulation world
        let world_viewer = WorldViewer::from_world(simulation.world())?;
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

    Ok(())
}

fn log_timestamp(io: &mut dyn Write) -> std::io::Result<()> {
    let tick = simulation::current_tick();
    write!(io, "T{:06}", tick)
}

fn main() {
    // enable structured logging before anything else
    let logger_guard = match logging::LoggerBuilder::with_env()
        .and_then(|builder| builder.init(log_timestamp, current_tick))
    {
        Err(e) => {
            eprintln!("failed to setup logging: {:?}", e);
            std::process::exit(1);
        }
        Ok(l) => l,
    };

    my_info!("initialized logging"; "level" => ?logger_guard.level());

    #[cfg(feature = "count-allocs")]
    let result = {
        use alloc_counter::count_alloc;
        let (counts, result) = count_alloc(|| do_main());
        // TODO more granular - n for engine setup, n for sim setup, n for each frame?
        my_info!(
            "{allocs} allocations, {reallocs} reallocs, {frees} frees",
            allocs = counts.0,
            reallocs = counts.1,
            frees = counts.2
        );
        result
    };

    #[cfg(not(feature = "count-allocs"))]
    let result = do_main();

    let exit = match result {
        Err(e) => {
            my_crit!("critical error"; "error" => %e);

            if logger().is_enabled(MyLevel::Debug) {
                my_crit!("more detail"; "error" => ?e);
            }

            // TODO use error chaining when stable (https://github.com/rust-lang/rust/issues/58520)
            let mut src = e.source();
            while let Some(source) = src {
                my_crit!("reason"; "cause" => %source, "debug" => ?source);
                src = source.source();
            }

            1
        }
        Ok(()) => 0,
    };

    my_info!("exiting cleanly"; "code"=>exit);

    drop(logger_guard); // flush
    std::process::exit(exit);
}
