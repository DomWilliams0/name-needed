use common::*;
use simulation::state::BackendState;
use simulation::{
    self, AsyncWorkerPool, Exit, InitializedSimulationBackend, PersistentSimulationBackend,
    Simulation, WorldPosition, WorldViewer,
};

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
    /// config file name within resources
    #[argh(option, default = "\"config.ron\".to_owned()")]
    config: String,

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
    let scenario = resolve_scenario(&args)?;

    // start metrics server
    #[cfg(feature = "metrics")]
    metrics::start_serving();

    // init resources root
    let resources = Resources::new(args.directory.canonicalize()?)?;

    // load config
    let config_filename = {
        let mut path = PathBuf::from(args.config);
        path.set_extension("ron");
        path.into_os_string()
    };
    info!("loading config"; "file" => ?config_filename);

    let resource_path = resources.get_file(&*config_filename)?;
    let file_path = resource_path
        .file_path()
        .expect("non file config not yet supported"); // TODO

    config::init(ConfigType::WatchedFile(file_path))?;

    // initialize persistent backend
    let mut backend_state = {
        log_scope!(o!("backend" => Backend::name()));
        BackendState::<Backend>::new(&resources)?
    };

    loop {
        let (simulation, initial_block) = start::create_simulation(resources.clone(), scenario)?;

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
                    testing::destroy_hook();
                    break Ok(());
                }
                HookResult::TestFailure(err) => {
                    error!("test failed: {}", err);
                    testing::destroy_hook();
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

mod start {
    use super::Renderer;
    use crate::scenarios::Scenario;
    use common::*;
    use config::WorldSource;
    use resources::Resources;
    use simulation::{
        all_slabs_in_range, presets, AsyncWorkerPool, ChunkLocation, GeneratedTerrainSource,
        PlanetParams, Simulation, SlabLocation, TerrainSourceError, ThreadedWorldLoader,
        WorldLoader, WorldPosition,
    };
    use std::time::Duration;

    /// (new empty simulation, initial block to centre camera on)
    pub fn create_simulation(
        resources: Resources,
        scenario: Scenario,
    ) -> BoxedResult<(Simulation<Renderer>, WorldPosition)> {
        // create world loader
        let mut world_loader = {
            let thread_count = config::get()
                .world
                .worker_threads
                .unwrap_or_else(|| (num_cpus::get() - 2).max(1));
            debug!(
                "using {threads} threads for world loader",
                threads = thread_count
            );
            let pool = AsyncWorkerPool::new(thread_count)?;

            let which_source = config::get().world.source.clone();
            world_from_source(which_source, pool, &resources.world_gen()?)?
        };

        let initial_block = load_initial_world(&mut world_loader)?;
        info!("centring camera on block"; "block" => %initial_block);

        let mut sim = Simulation::new(world_loader, resources)?;
        init_simulation(&mut sim, scenario)?;
        Ok((sim, initial_block))
    }

    fn world_from_source(
        source: config::WorldSource,
        pool: AsyncWorkerPool,
        resources: &resources::WorldGen,
    ) -> BoxedResult<WorldLoader<simulation::WorldContext>> {
        Ok(match source {
            config::WorldSource::Preset(preset) => {
                debug!("loading world preset"; "preset" => ?preset);
                let source = presets::from_preset(preset);
                WorldLoader::new(source, pool)
            }
            config::WorldSource::Generate(file) => {
                debug!("generating world from config"; "path" => %file.display());

                let params = PlanetParams::load_with_only_file(resources, file.as_os_str());
                let source = params.and_then(|params| {
                    pool.runtime()
                        .block_on(async { GeneratedTerrainSource::new(params).await })
                })?;
                WorldLoader::new(source, pool)
            }
        })
    }

    fn load_initial_world(
        world_loader: &mut WorldLoader<simulation::WorldContext>,
    ) -> BoxedResult<WorldPosition> {
        let (chunk, slab_depth, chunk_radius, is_preset) = {
            let cfg = config::get();
            let (cx, cy) = match cfg.world.source {
                WorldSource::Preset(_) => (0, 0), // ignore config for preset worlds
                WorldSource::Generate(_) => cfg.world.initial_chunk,
            };

            (
                ChunkLocation(cx, cy),
                cfg.world.initial_slab_depth as i32,
                cfg.world.initial_chunk_radius as i32,
                cfg.world.source.is_preset(),
            )
        };

        // request ground level in requested start chunk
        // TODO middle of requested chunk instead of corner
        let ground_level = {
            let block = chunk.get_block(0); // z ignored
            match world_loader.get_ground_level(block) {
                Ok(slice) => slice,
                Err(TerrainSourceError::BlockOutOfBounds(_)) if is_preset => {
                    // special case, assume preset starts at 0
                    warn!(
                        "could not find block {:?} in preset world, assuming ground is at 0",
                        block
                    );
                    0.into()
                }
                err => err?,
            }
        };

        let ground_slab = ground_level.slab_index();

        debug!(
            "ground level in {chunk:?} is {ground}",
            chunk = chunk,
            ground = ground_level.slice();
            ground_slab,
        );

        let initial_block = chunk.get_block(ground_level);

        // request slab range and wait for completion or timeout
        let (slabs_to_request, slab_count) = all_slabs_in_range(
            SlabLocation::new(
                ground_slab - slab_depth,
                (chunk.x() - chunk_radius, chunk.y() - chunk_radius),
            ),
            SlabLocation::new(
                ground_slab + slab_depth,
                (chunk.x() + chunk_radius, chunk.y() + chunk_radius),
            ),
        );

        debug!(
            "waiting for world to load {slabs} slabs around chunk {chunk:?} \
                before initializing simulation",
            chunk = chunk,
            slabs = slab_count
        );

        world_loader.request_slabs_with_count(slabs_to_request, slab_count);
        let timeout = Duration::from_secs(config::get().world.load_timeout as u64);
        world_loader.block_for_last_batch_with_bail(timeout, panik::has_panicked)?;

        Ok(initial_block)
    }

    fn init_simulation(sim: &mut Simulation<Renderer>, scenario: Scenario) -> BoxedResult<()> {
        let (seed, source) = if let Some(seed) = config::get().simulation.random_seed {
            (seed, "config")
        } else {
            (thread_rng().next_u64(), "randomly generated")
        };

        random::reseed(seed);
        info!(
            "seeding random generator with seed {seed}",
            seed = seed; "source" => source
        );

        // create society for player to control
        let player_society = sim
            .societies()
            .new_society("Top Geezers".to_owned())
            .unwrap();
        *sim.player_society() = Some(player_society);

        // defer to scenario for entity spawning
        scenario(&sim.world());
        Ok(())
    }
}
