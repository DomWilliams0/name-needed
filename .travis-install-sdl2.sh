
#!/usr/bin/env bash

set -xueo pipefail

wget https://www.libsdl.org/release/SDL2-2.0.9.tar.gz -O sdl2.tar.gz
tar xzf sdl2.tar.gz
pushd SDL2-* && ./configure && make -j`nproc` && sudo make install && popd
