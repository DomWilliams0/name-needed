#!/bin/bash

set -e

pushd renderer/main
cargo run --verbose --features lite --no-default-features -- --preset ci --directory ../../ --scenario wander_and_eat
popd

pushd game/procgen
cargo run --verbose --package procgen --bin procgen --features bin
popd
