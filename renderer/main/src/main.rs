use common::*;
use presets::{DevGamePreset, EmptyGamePreset, GamePreset};
use simulation::state::BackendState;
use simulation::{
    self, Exit, InitializedSimulationBackend, PersistentSimulationBackend, WorldViewer,
};

use crate::presets::ContinuousIntegrationGamePreset;

use engine::panic::Panic;
use engine::Engine;
use resources::resource::Resources;
use resources::ResourceContainer;
use std::io::Write;
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
mod scenarios;

#[cfg(feature = "use-sdl")]
type Backend = engine::SdlBackendPersistent;

#[cfg(feature = "lite")]
type Backend = engine::DummyBackendPersistent;

type BackendInit = <Backend as PersistentSimulationBackend>::Initialized;
type Renderer = <BackendInit as InitializedSimulationBackend>::Renderer;

/// 🎵 Nice game without a name 🎵
#[derive(argh::FromArgs)]
struct Args {
    /// game preset
    #[argh(option)]
    preset: Option<String>,

    /// directory containing game files
    #[argh(option, default = "PathBuf::from(\".\")")]
    directory: PathBuf,

    /// scenario to load
    #[argh(option)]
    scenario: Option<String>,
}

#[derive(Debug, Error)]
enum StartError {
    #[error("No such scenario '{0}'")]
    NoSuchScenario(String),

    #[error("No such preset '{0}'")]
    NoSuchPreset(String),
}

fn do_main() -> BoxedResult<()> {
    let args = argh::from_env::<Args>();

    let preset: Box<dyn GamePreset<Renderer>> = match args.preset.as_deref() {
        None | Some("dev") => Box::new(DevGamePreset::<Renderer>::default()),
        Some("ci") => Box::new(ContinuousIntegrationGamePreset::default()),
        Some("empty") => Box::new(EmptyGamePreset::default()),
        Some(other) => return Err(StartError::NoSuchPreset(other.to_owned()).into()),
    };

    info!("chosen game preset"; "preset" => ?preset.name());

    let scenario = {
        let name = args.scenario.as_deref();
        let resolved = scenarios::resolve(name);
        match resolved {
            Some((name, s)) => {
                info!(
                    "resolved scenario '{scenario} to {function:#x}'",
                    scenario = name,
                    function = s as usize
                );
                s
            }
            None => {
                let name = name.unwrap(); // would have panicked already if bad default
                error!("failed to resolve scenario"; "name" => ?name);

                let possibilities = scenarios::all_names().collect_vec();
                info!("available scenarios: {:?}", possibilities);

                return Err(StartError::NoSuchScenario(name.to_owned()).into());
            }
        }
    };

    // start metrics server
    #[cfg(feature = "metrics")]
    metrics::start_serving();

    // init resources root
    let resources = Resources::new(args.directory)?;

    // load config
    if let Some(config_file_name) = preset.config() {
        let config_path = resources.get_file(config_file_name)?;

        info!("loading config"; "path" => ?config_path);
        config::init(config_path)?;
    }

    // initialize persistent backend
    let mut backend_state = {
        log_scope!(o!("backend" => Backend::name()));
        BackendState::<Backend>::new(&resources)?
    };

    loop {
        // create simulation
        let simulation = preset.load(resources.clone(), scenario)?;

        // initialize backend with simulation world
        let world_viewer = WorldViewer::with_world(simulation.voxel_world())?;
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
    let logger_guard =
        match logging::LoggerBuilder::with_env().and_then(|builder| builder.init(log_timestamp)) {
            Err(e) => {
                eprintln!("failed to setup logging: {:?}", e);
                std::process::exit(1);
            }
            Ok(l) => l,
        };

    info!("initialized logging"; "level" => ?logger_guard.level());

    engine::panic::init_panic_detection();

    let result = std::panic::catch_unwind(|| {
        #[cfg(feature = "count-allocs")]
        {
            use alloc_counter::count_alloc;
            let (counts, result) = count_alloc(|| do_main());
            // TODO more granular - n for engine setup, n for sim setup, n for each frame?
            info!(
                "{allocs} allocations, {reallocs} reallocs, {frees} frees",
                allocs = counts.0,
                reallocs = counts.1,
                frees = counts.2
            );
            result
        }

        #[cfg(not(feature = "count-allocs"))]
        do_main()
    });

    let all_panics = engine::panic::panics().collect_vec();

    let exit = match result {
        _ if !all_panics.is_empty() => {
            crit!("{count} threads panicked", count = all_panics.len());

            for Panic {
                message,
                thread,
                mut backtrace,
            } in all_panics
            {
                backtrace.resolve();

                crit!("panic";
                "backtrace" => ?backtrace,
                "message" => message,
                "thread" => thread,
                );
            }

            1
        }
        Err(_) => {
            // panics are caught by the case above
            unreachable!()
        }

        Ok(Err(e)) => {
            crit!("critical error"; "error" => %e);

            if logger().is_enabled(MyLevel::Debug) {
                crit!("more detail"; "error" => ?e);
            }

            // TODO use error chaining when stable (https://github.com/rust-lang/rust/issues/58520)
            let mut src = e.source();
            while let Some(source) = src {
                crit!("reason"; "cause" => %source, "debug" => ?source);
                src = source.source();
            }

            1
        }
        Ok(Ok(())) => 0,
    };

    info!("exiting cleanly"; "code"=>exit);

    // unhook custom panic handler before dropping and flushing the logger
    let _ = std::panic::take_hook();
    drop(logger_guard);

    std::process::exit(exit);
}
