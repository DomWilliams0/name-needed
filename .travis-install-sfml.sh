#!/usr/bin/env bash

set -xueo pipefail

CSFML_VERSION=2.5
SFML_VERSION=2.5.1

if [ ! -d "$SFML_INSTALL/usr/local/lib" ]
then
  wget http://www.sfml-dev.org/files/SFML-$SFML_VERSION-sources.zip #--no-check-certificate
  unzip -q SFML-$SFML_VERSION-sources.zip
  pushd SFML-$SFML_VERSION && mkdir build && cd build && cmake .. && make
  make DESTDIR="$SFML_INSTALL" install
  popd

  wget http://www.sfml-dev.org/files/CSFML-$CSFML_VERSION-sources.zip --no-check-certificate
  unzip -q CSFML-$CSFML_VERSION-sources.zip
  pushd CSFML-$CSFML_VERSION
  mkdir build && cd build && cmake -DSFML_DIR="$SFML_INSTALL/usr/local/lib/cmake/SFML/" .. && make
  make DESTDIR="$SFML_INSTALL" install
  popd

fi
