#!/bin/bash

set -e

pushd renderer/main
cargo run --verbose --features lite --features procgen --no-default-features -- --config ci_test --directory ../../ --scenario wander_and_eat
popd

pushd game/procgen
cargo run --verbose --package procgen --bin procgen --features bin
popd
