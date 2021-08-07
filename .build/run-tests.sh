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
RUN_E2E_TESTS=0
if [[ "$DETECTED_OS" != "Linux" ]]; then
	RUN_E2E_TESTS=1

	# remove testing crate from workspace for windows
	sed -i '/"testing",/d' Cargo.toml
	sed -i 's/] # end members/]\nexclude = ["testing"]/g' Cargo.toml
fi

cat Cargo.toml

cargo test $FLAGS
cargo test $FLAGS -- --ignored

if [[ $RUN_E2E_TESTS ]]; then
	cargo run --bin test-runner
else
	echo "skipping e2e tests for platform $DETECTED_OS"
fi
