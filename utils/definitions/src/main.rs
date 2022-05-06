use std::io;

use logging::prelude::*;

fn dont_log_time(_: &mut dyn io::Write) -> io::Result<()> {
    Ok(())
}

fn main() {
    let logger =
        match logging::LoggerBuilder::with_env().and_then(|builder| builder.init(dont_log_time)) {
            Err(e) => {
                eprintln!("failed to setup logging: {:?}", e);
                std::process::exit(1);
            }
            Ok(l) => l,
        };

    info!("initialized logging"; "level" => ?logger.level());

    if do_it().is_err() {
        std::process::exit(1);
    }
}

fn do_it() -> Result<(), ()> {
    let game_dir = ".";

    let res = resources::Resources::new(game_dir).expect("bad game dir");
    let def_res = res
        .definitions()
        .expect("missing definitions dir in resources");

    let strings = Default::default();
    let loaded = match simulation::load_definitions(def_res, &strings) {
        Ok(reg) => reg,
        Err(errs) => {
            error!("failed to load definitions: {}", errs);
            return Err(());
        }
    };

    for (name, def) in loaded.iter() {
        info!("{}: {:#?}", name, def);
    }

    Ok(())
}
