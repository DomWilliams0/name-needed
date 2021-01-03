use common::*;
use procgen::*;
use std::io::Write;

// TODO actually log the time
fn log_time(out: &mut dyn Write) -> std::io::Result<()> {
    write!(out, "the time")
}

#[cfg(feature = "bin")]
fn main() {
    // parse config and args first
    let params = PlanetParams::load_with_args("procgen.txt");

    let _logging = logging::LoggerBuilder::with_env()
        .and_then(|builder| builder.init(log_time))
        .expect("logging failed");
    info!("initialized logging"; "level" => ?_logging.level());

    let exit = match params {
        Err(err) => {
            error!("failed to parse params: {}", err);
            1
        }
        Ok(params) if params.log_params_and_exit => {
            // nop
            info!("config: {:#?}", params);
            0
        }
        Ok(params) => {
            debug!("config: {:#?}", params);
            common::panic::init_panic_detection();

            let dew_it = || {
                let mut planet = Planet::new(params).expect("failed");
                planet.initial_generation();

                let mut render = Render::with_planet(planet.clone());
                render.draw_continents();
                render.save("procgen.png").expect("failed to write image");

                let y = 50;
                for x in 50..=52 {
                    planet.realize_region((x, y));

                    let mut render = Render::with_planet(planet.clone());
                    render.draw_region((x, y));
                    render
                        .save(format!("procgen-region-{}-{}.png", x, y))
                        .expect("failed to write image");
                }
            };

            match common::panic::run_and_handle_panics(dew_it) {
                Ok(_) => 0,
                Err(_) => 1,
            }
        }
    };

    // let logging end gracefully
    info!("all done");
    drop(_logging);
    std::thread::sleep(std::time::Duration::from_secs(1));

    std::process::exit(exit);
}

#[cfg(not(feature = "bin"))]
fn main() {
    unreachable!("missing feature \"bin\"")
}
