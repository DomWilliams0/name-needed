use std::io;

use structopt::StructOpt;

use logging::prelude::*;
use simulation::Definition;

fn dont_log_time(_: &mut dyn io::Write) -> io::Result<()> {
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Params {
    /// Filter by definition category
    #[structopt(long, short)]
    category: Option<String>,

    /// Filter by definition uid
    #[structopt(name = "FILTER")]
    filter: Option<String>,
}

fn main() {
    let logger = match logging::LoggerBuilder::with_env("RUST_LOG")
        .and_then(|builder| builder.init(dont_log_time))
    {
        Err(e) => {
            eprintln!("failed to setup logging: {:?}", e);
            std::process::exit(1);
        }
        Ok(l) => l,
    };

    info!("initialized logging"; "level" => ?logger.level());

    let params = Params::from_args();

    if do_it(params).is_err() {
        drop(logger); // ensure flushed
        std::process::exit(1);
    }
}

fn do_it(params: Params) -> Result<(), ()> {
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

    for (name, def, _) in loaded.iter().filter(|def| params.should_include(def)) {
        info!("{}: {:#?}", name, def);
    }

    Ok(())
}

impl Params {
    fn should_include(&self, (name, _, category): &(&str, &Definition, Option<String>)) -> bool {
        // fail open if no filters
        if self.filter.is_none() && self.category.is_none() {
            return true;
        }

        if let Some(filter) = self.filter.as_ref() {
            if !name.contains(filter) {
                return false;
            }
        }

        if let Some(filter) = self.category.as_ref() {
            match category {
                Some(cat) if cat.contains(filter) => { /* nice */ }
                _ => return false,
            }
        }

        true
    }
}
