#!/bin/bash
set -e

export RUST_BACKTRACE=1
export CARGO_INCREMENTAL=0
export NN_LOG=debug
export NN_TEST_WORLD_TIMEOUT=180 # seconds

# avoid SDL dependency by not running renderer tests
FLAGS="--verbose --workspace --exclude engine --exclude main"

# TODO fix "LNK1189: library limit of 65535 objects exceeded" on windows when building `testing` crate
DETECTED_OS=$(uname)

if [[ "$DETECTED_OS" == "Linux" ]]; then
	RUN_E2E_TESTS=1
else
	RUN_E2E_TESTS=0

	# remove testing crate from workspace for windows as it doesn't compile
	sed -i '/"testing",/d' Cargo.toml
	sed -i 's/] # end members/]\nexclude = ["testing"]/g' Cargo.toml
fi

echo "OS is $DETECTED_OS"
echo "running e2e tests: $RUN_E2E_TESTS"

cargo test $FLAGS
cargo test $FLAGS -- --ignored

if (( $RUN_E2E_TESTS )); then
	cargo run --bin test-runner --features testing
else
	echo "skipping e2e tests for platform $DETECTED_OS"
fi
