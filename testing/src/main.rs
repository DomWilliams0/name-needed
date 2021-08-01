use futures::TryFutureExt;
use std::error::Error;
use std::process::Stdio;
use std::time::Duration;
use testing::{register_tests, TestDeclaration, TEST_NAME_VAR};
use tokio::process::{Child, Command};

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

#[tokio::main(flavor = "multi_thread", worker_threads = 5)]
async fn main() {
    if let Err(err) = do_main().await {
        panic!("{}", err)
    }
}

async fn do_main() -> Result<(), Box<dyn Error>> {
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

    CargoCommand::Build
        .run(renderer)
        .and_then(wait_on_process)
        .await?;

    for test in tests {
        eprintln!("running test {:?}", test.name);
        // TODO run n in parallel

        let mut test_process = CargoCommand::Run { test: test.name }.run(renderer).await?;
        let result_fut = wait_on_process_ref(&mut test_process);

        let err = match tokio::time::timeout(Duration::from_secs(args.timeout as u64), result_fut)
            .await
        {
            Ok(Ok(_)) => {
                eprintln!("test {} passed", test.name);
                continue;
            }
            Ok(Err(err)) => format!("{}", err),
            Err(_) => "timed out".to_owned(),
        };

        let msg = format!("test {} failed: {}", test.name, err);
        eprintln!("{}", msg);

        // abort test process
        test_process.kill().await?;
        std::process::exit(1);
    }

    eprintln!("done");
    Ok(())
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
    async fn run(self, renderer: Renderer) -> Result<Child, Box<dyn Error>> {
        let subcmd = match &self {
            CargoCommand::Build => "build",
            CargoCommand::Run { .. } => "run",
        };

        let mut builder = Command::new(env!("CARGO"));
        if let CargoCommand::Run { test } = self {
            builder.env(TEST_NAME_VAR, test);
        }
        builder
            .args(&[
                subcmd,
                "--bin",
                "main",
                "--no-default-features",
                "--features",
                "tests",
            ])
            .stdin(Stdio::piped());

        let feature = match renderer {
            Renderer::Lite => "lite",
            Renderer::Graphical => "use-sdl",
        };
        builder
            .args(&["--features", feature])
            .spawn()
            .map_err(Into::into)
    }
}

async fn wait_on_process(mut process: Child) -> Result<(), Box<dyn Error>> {
    wait_on_process_ref(&mut process).await
}

async fn wait_on_process_ref(process: &mut Child) -> Result<(), Box<dyn Error>> {
    let result = process.wait().await?;
    if result.success() {
        Ok(())
    } else {
        // TODO unix special case to get exit code on signal
        Err(format!("cargo exited with code {:?}", result.code()).into())
    }
}
