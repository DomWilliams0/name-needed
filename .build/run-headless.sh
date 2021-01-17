#!/bin/bash

set -e

cd renderer/main
cargo run --verbose --features lite --no-default-features -- --preset ci --directory ../../ --scenario wander_and_eat
