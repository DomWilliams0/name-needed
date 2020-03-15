#!/usr/bin/env bash

set -xueo pipefail

if [ ! -d "$BULLET_DIR/build" ]; then
  wget https://github.com/bulletphysics/bullet3/archive/2.89.tar.gz -O bullet.tar.gz
  tar xzf bullet.tar.gz
  mkdir "$BULLET_DIR/build"
fi

pushd "$BULLET_DIR/build"
cmake .. -DBUILD_BULLET2_DEMOS=0 -DBUILD_CPU_DEMOS=0 -DBUILD_OPENGL3_DEMOS=0 -DBUILD_UNIT_TESTS=0 -DUSE_GLUT=0 \
         -DUSE_GRAPHICAL_BENCHMARK=0 -DBUILD_SHARED_LIBS=1
sudo make install
popd
