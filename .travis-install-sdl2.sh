#!/usr/bin/env bash

set -xueo pipefail

SDL_VERSION=2.0.9

if [ ! -f "$SDL_DIR/configure" ]
then
	wget https://www.libsdl.org/release/SDL2-$SDL_VERSION.tar.gz -O sdl2.tar.gz
	tar xzf sdl2.tar.gz
fi

pushd $SDL_DIR && ./configure && make -j`nproc` && sudo make install && popd
