#!/bin/bash
set -e

export RUST_BACKTRACE=1
export NN_LOG=debug
export NN_TEST_WORLD_TIMEOUT=180 # seconds

# avoid SDL dependency by not running renderer tests
FLAGS="--verbose --workspace --exclude engine --exclude main"

cargo test $FLAGS
cargo test $FLAGS -- --ignored
