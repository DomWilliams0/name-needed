#!/bin/bash
set -e

export RUST_BACKTRACE=1
export NN_LOG=debug

cargo test --verbose --workspace
cargo test --verbose -- --ignored
