use common::*;
use presets::{DevGamePreset, EmptyGamePreset, GamePreset};
use simulation::state::BackendState;
use simulation::{
    self, Exit, InitializedSimulationBackend, PersistentSimulationBackend, WorldViewer,
};

use crate::presets::ContinuousIntegrationGamePreset;

use crate::scenarios::Scenario;
use config::ConfigType;
use engine::Engine;
use resources::ResourceContainer;
use resources::Resources;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

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

    #[cfg(not(feature = "tests"))]
    /// scenario to load
    #[argh(option)]
    scenario: Option<String>,
    // TODO specify e2e test by name (feature = "tests")
}

#[derive(Debug, Error)]
enum StartError {
    #[error("No such scenario '{0}'")]
    NoSuchScenario(String),

    #[error("No such preset '{0}'")]
    NoSuchPreset(String),
}

fn resolve_scenario(args: &Args) -> BoxedResult<Scenario> {
    let name;
    #[cfg(feature = "tests")]
    {
        name = Some("nop");
    }
    #[cfg(not(feature = "tests"))]
    {
        name = args.scenario.as_deref();
    }

    let resolved = scenarios::resolve(name);
    match resolved {
        Some((name, s)) => {
            info!(
                "resolved scenario '{scenario}' to {:#x}",
                s as usize,
                scenario = name,
            );
            Ok(s)
        }
        None => {
            let name = name.unwrap(); // would have panicked already if bad default
            error!("failed to resolve scenario"; "name" => ?name);

            let possibilities = scenarios::all_names().collect_vec();
            info!("available scenarios: {:?}", possibilities);

            return Err(StartError::NoSuchScenario(name.to_owned()).into());
        }
    }
}

#[allow(unused_mut)]
fn do_main() -> BoxedResult<()> {
    let args = argh::from_env::<Args>();

    let preset: Box<dyn GamePreset<Renderer>> = match args.preset.as_deref() {
        None | Some("dev") => Box::new(DevGamePreset::<Renderer>::default()),
        Some("ci") => Box::new(ContinuousIntegrationGamePreset::default()),
        Some("empty") => Box::new(EmptyGamePreset::default()),
        Some(other) => return Err(StartError::NoSuchPreset(other.to_owned()).into()),
    };

    info!("chosen game preset"; "preset" => ?preset.name());

    let scenario = resolve_scenario(&args)?;

    // start metrics server
    #[cfg(feature = "metrics")]
    metrics::start_serving();

    // init resources root
    let resources = Resources::new(args.directory.canonicalize()?)?;

    // load config
    if let Some(config_file_name) = preset.config_filename() {
        info!("loading config"; "file" => ?config_file_name);

        let resource_path = resources.get_file(config_file_name)?;
        let file_path = resource_path
            .file_path()
            .expect("non file config not yet supported"); // TODO

        config::init(ConfigType::WatchedFile(file_path))?;
    }

    // initialize persistent backend
    let mut backend_state = {
        log_scope!(o!("backend" => Backend::name()));
        BackendState::<Backend>::new(&resources)?
    };

    loop {
        // create simulation
        let (simulation, initial_block) = preset.load(resources.clone(), scenario)?;

        // initialize backend with simulation world
        let world_viewer = WorldViewer::with_world(simulation.voxel_world(), initial_block)?;
        let backend = backend_state.start(world_viewer, initial_block);

        // initialize engine
        let mut engine = Engine::new(simulation, backend);

        #[cfg(feature = "tests")]
        {
            use testing::HookResult;
            engine.set_tick_hook(Some(testing::tick_hook));
            info!("running test init hook");
            let ctx = engine.hook_context();
            match testing::init_hook(&ctx) {
                HookResult::KeepGoing => {}
                HookResult::TestSuccess => {
                    info!("test finished successfully");
                    break Ok(());
                }
                HookResult::TestFailure(err) => {
                    error!("test failed: {}", err);
                    break Err(err.into());
                }
            }
        }

        // run game until exit
        let exit = engine.run();

        // uninitialize backend
        backend_state.end();

        match exit {
            Exit::Stop => break Ok(()),
            Exit::Abort(err) => break Err(err.into()),
            Exit::Restart => continue,
        }
    }
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

    let result = panik::Builder::new()
        .slogger(logger_guard.logger())
        .run_and_handle_panics(|| {
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

    let exit = match result {
        None => {
            // panic handled above
            1
        }
        Some(Err(e)) => {
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
        Some(Ok(())) => 0,
    };

    // unhook custom panic handler before dropping and flushing the logger
    let _ = std::panic::take_hook();
    if exit != 0 {
        const SECONDS: u64 = 2;
        info!(
            "waiting {seconds} seconds to allow other threads to finish logging",
            seconds = SECONDS
        );
        std::thread::sleep(Duration::from_secs(SECONDS));
    }
    info!("exiting cleanly"; "code" => exit);
    drop(logger_guard);

    std::process::exit(exit);
}
