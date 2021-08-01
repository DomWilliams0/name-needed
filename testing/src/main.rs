use std::error::Error;
use std::process::Command;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread::spawn;
use std::time::Duration;
use testing::{register_tests, TestDeclaration, TEST_NAME_VAR};

/// Test runner
#[derive(argh::FromArgs)]
struct Args {
    /// if graphical renderer should be used
    #[argh(switch)]
    graphical: bool,

    // TODO specify single test to run
    /// timeout in seconds per test
    #[argh(option, default = "30")]
    timeout: u32,
}

fn main() {
    register_tests();

    let args = argh::from_env::<Args>();
    let renderer = if args.graphical {
        Renderer::Graphical
    } else {
        Renderer::Lite
    };

    let tests = inventory::iter::<TestDeclaration>
        .into_iter()
        .collect::<Vec<_>>();
    eprintln!("running {} tests", tests.len());

    CargoCommand::Build.run(renderer).expect("failed to build");
    for test in tests {
        eprintln!("running test {:?}", test.name);
        // TODO run n in parallel

        let (tx, rx) = channel();
        let _ = spawn(move || {
            let result = CargoCommand::Run { test: test.name }.run(renderer);
            tx.send(result.map_err(|err| format!("{}", err)))
                .expect("failed to send result");
        });

        let error = match rx.recv_timeout(Duration::from_secs(args.timeout as u64)) {
            Ok(Ok(_)) => {
                eprintln!("test {} passed", test.name);
                continue;
            }
            Ok(Err(err)) => err,
            Err(_) => "timed out".to_owned(),
        };

        // TODO test thread needs to exit game
        panic!("test {} failed: {}", test.name, error);
    }

    eprintln!("done")
}

enum CargoCommand<'a> {
    Build,
    Run { test: &'a str },
}

#[derive(Copy, Clone)]
enum Renderer {
    Lite,
    Graphical,
}

impl CargoCommand<'_> {
    fn run(self, renderer: Renderer) -> Result<(), Box<dyn Error>> {
        let subcmd = match &self {
            CargoCommand::Build => "build",
            CargoCommand::Run { .. } => "run",
        };

        let mut process = {
            let mut builder = Command::new(env!("CARGO"));
            if let CargoCommand::Run { test } = self {
                builder.env(TEST_NAME_VAR, test);
            }
            builder.args(&[
                subcmd,
                "--bin",
                "main",
                "--no-default-features",
                "--features",
                "tests",
            ]);

            let feature = match renderer {
                Renderer::Lite => "lite",
                Renderer::Graphical => "use-sdl",
            };
            builder.args(&["--features", feature]);

            builder.spawn()?
        };

        let result = process.wait()?;
        if result.success() {
            Ok(())
        } else {
            // TODO unix special case to get exit code on signal
            Err(format!("cargo exited with code {:?}", result.code()).into())
        }
    }
}
