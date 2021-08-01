#!/bin/bash
set -e

export RUST_BACKTRACE=1
export CARGO_INCREMENTAL=0
export NN_LOG=debug
export NN_TEST_WORLD_TIMEOUT=180 # seconds

# avoid SDL dependency by not running renderer tests
FLAGS="--verbose --workspace --exclude engine --exclude main"

cargo test $FLAGS
cargo test $FLAGS -- --ignored

# TODO fix "LNK1189: library limit of 65535 objects exceeded" on windows
if [[ "$RUNNER_OS" != "Windows" ]]; then
	cargo run --bin test-runner
fi
